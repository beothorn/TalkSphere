//! Bounded line-delimited JSON requests and replies for local node control.
//!
//! Callers: the control server, its command handler and the `node-ctl` client. Every
//! request is one JSON object on one line with an exact unsigned `id`, an `op` and
//! operation parameters; every reply echoes the `id` and is either `ok` with a `result`
//! or an explicit error with a code and a message that never contains secrets or payload.
//! Integers are exact unsigned JSON integers; byte values travel as lowercase hex.
//! This is a TalkSphere operator interface, not the CR2SE Node API.
use serde_json::{Map, Value, json};
use std::io::Read;

/// Failure classes a client can act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    /// The request is malformed or a parameter is invalid.
    Invalid,
    /// The operation or option is not supported.
    Unsupported,
    /// The caller or peer was not authorized, or the lease is unknown.
    Denied,
    /// The peer refused before any commitment; nothing was charged.
    Refused,
    /// Credits or an explicit spending bound are insufficient.
    Insufficient,
    /// A local dependency or the peer is unavailable.
    Unavailable,
    /// A finite limit was reached.
    Limit,
    /// The remote effect may have committed; the work is not retried.
    UnknownOutcome,
    /// The node is stopping or too busy.
    Busy,
}
impl Code {
    /// Stable wire spelling.
    pub fn name(self) -> &'static str {
        tracing::trace!("Naming a control error code");
        match self {
            Self::Invalid => "invalid",
            Self::Unsupported => "unsupported",
            Self::Denied => "denied",
            Self::Refused => "refused",
            Self::Insufficient => "insufficient",
            Self::Unavailable => "unavailable",
            Self::Limit => "limit",
            Self::UnknownOutcome => "unknown_outcome",
            Self::Busy => "busy",
        }
    }
}

/// An explicit refusal with a short fixed-vocabulary message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlError {
    /// Failure class.
    pub code: Code,
    /// Human-readable explanation without secrets, payload or peer text.
    pub message: String,
}
impl ControlError {
    /// Build an error from a code and message.
    pub fn new(code: Code, message: impl Into<String>) -> Self {
        tracing::trace!("Building a control error");
        Self {
            code,
            message: message.into(),
        }
    }
}

/// One parsed request.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// Exact caller-chosen identifier echoed in the reply; also the idempotency key.
    pub id: u64,
    /// Operation name.
    pub op: String,
    /// Remaining members as parameters.
    pub params: Map<String, Value>,
}
impl Request {
    /// Read a required exact unsigned integer parameter.
    /// # Errors
    /// Missing, negative, fractional or non-numeric values are `Invalid`.
    pub fn u64(&self, name: &str) -> Result<u64, ControlError> {
        tracing::trace!("Reading an integer control parameter");
        self.params
            .get(name)
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                ControlError::new(Code::Invalid, format!("{name} must be an unsigned integer"))
            })
    }
    /// Read an optional exact unsigned integer parameter.
    /// # Errors
    /// A present value that is not an unsigned integer is `Invalid`.
    pub fn optional_u64(&self, name: &str) -> Result<Option<u64>, ControlError> {
        tracing::trace!("Reading an optional integer control parameter");
        match self.params.get(name) {
            None => Ok(None),
            Some(_) => self.u64(name).map(Some),
        }
    }
    /// Read a required text parameter.
    /// # Errors
    /// Missing or non-text values are `Invalid`.
    pub fn text(&self, name: &str) -> Result<&str, ControlError> {
        tracing::trace!("Reading a text control parameter");
        self.params
            .get(name)
            .and_then(Value::as_str)
            .ok_or_else(|| ControlError::new(Code::Invalid, format!("{name} must be text")))
    }
    /// Read an optional text parameter.
    /// # Errors
    /// A present value that is not text is `Invalid`.
    pub fn optional_text(&self, name: &str) -> Result<Option<&str>, ControlError> {
        tracing::trace!("Reading an optional text control parameter");
        match self.params.get(name) {
            None => Ok(None),
            Some(_) => self.text(name).map(Some),
        }
    }
    /// Read a required lowercase-hex byte parameter of at most `maximum` bytes.
    /// # Errors
    /// Bad hex or an oversized value is `Invalid`.
    pub fn bytes(&self, name: &str, maximum: usize) -> Result<Vec<u8>, ControlError> {
        tracing::trace!("Reading a hex control parameter");
        let bad = || {
            ControlError::new(
                Code::Invalid,
                format!("{name} must be lowercase hex within its bound"),
            )
        };
        let text = self.text(name)?;
        if text.len() > maximum.saturating_mul(2) {
            return Err(bad());
        }
        unhex(text).ok_or_else(bad)
    }
}

/// Parse one request line (without its newline) within `maximum` bytes.
/// # Errors
/// Oversized, non-JSON, non-object lines, missing or non-integer ids and missing operations are `Invalid`.
pub fn parse(line: &[u8], maximum: usize) -> Result<Request, ControlError> {
    tracing::trace!("Parsing a control request");
    let invalid = |message: &str| ControlError::new(Code::Invalid, message);
    if line.len() > maximum {
        return Err(invalid("request line is too long"));
    }
    let Value::Object(mut members) =
        serde_json::from_slice(line).map_err(|_| invalid("request is not a JSON object"))?
    else {
        return Err(invalid("request is not a JSON object"));
    };
    let id = members
        .remove("id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| invalid("id must be an unsigned integer"))?;
    let Some(Value::String(op)) = members.remove("op") else {
        return Err(invalid("op must be text"));
    };
    Ok(Request {
        id,
        op,
        params: members,
    })
}

/// Encode a success reply as one line.
pub fn ok(id: u64, result: Value) -> Vec<u8> {
    tracing::trace!("Encoding a control success reply");
    line(json!({"id": id, "ok": true, "result": result}))
}

/// Encode an error reply as one line.
pub fn error(id: u64, error: &ControlError) -> Vec<u8> {
    tracing::trace!("Encoding a control error reply");
    line(
        json!({"id": id, "ok": false, "error": {"code": error.code.name(), "message": error.message}}),
    )
}

// Serialize a reply object followed by the line terminator.
fn line(value: Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&value).unwrap_or_else(|_| b"{}".to_vec());
    bytes.push(b'\n');
    bytes
}

/// Outcome of reading one line from a stream.
#[derive(Debug, PartialEq, Eq)]
pub enum Line {
    /// A complete line without its terminator.
    Complete(Vec<u8>),
    /// The peer closed before sending a full line.
    Closed,
    /// The line exceeded the bound; the connection must be closed.
    TooLong,
}

/// Read one newline-terminated line of at most `maximum` bytes, byte by byte through `reader`.
/// A read error other than interruption is reported as closed.
/// `on_timeout` is consulted when a read times out and returns whether to keep waiting.
pub fn read_line(
    reader: &mut impl Read,
    maximum: usize,
    mut on_timeout: impl FnMut() -> bool,
) -> Line {
    tracing::trace!("Reading one bounded control line");
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match reader.read(&mut byte) {
            Ok(0) => return Line::Closed,
            Ok(_) if byte[0] == b'\n' => return Line::Complete(line),
            Ok(_) => {
                if line.len() >= maximum {
                    return Line::TooLong;
                }
                line.push(byte[0]);
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                if !on_timeout() {
                    return Line::Closed;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Line::Closed,
        }
    }
}

/// Lowercase hexadecimal text of bytes.
pub fn hex(bytes: &[u8]) -> String {
    tracing::trace!("Encoding hexadecimal text");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Decode strict lowercase hexadecimal text.
pub fn unhex(text: &str) -> Option<Vec<u8>> {
    tracing::trace!("Decoding hexadecimal text");
    if !text.len().is_multiple_of(2)
        || !text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    // Requests parse into id, operation and parameters; exact integers survive.
    #[test]
    fn parses_requests_with_exact_integers() {
        let request = parse(
            br#"{"id":18446744073709551615,"op":"grant","amount":9007199254740993,"peer":"x"}"#,
            1024,
        )
        .unwrap();
        assert_eq!(request.id, u64::MAX);
        assert_eq!(request.op, "grant");
        assert_eq!(request.u64("amount").unwrap(), 9_007_199_254_740_993);
        assert_eq!(request.text("peer").unwrap(), "x");
        assert!(request.optional_u64("missing").unwrap().is_none());
    }
    // Malformed, oversized and mistyped input is refused with an invalid code and no echo of the input.
    #[test]
    fn rejects_malformed_requests() {
        for text in [
            "",
            "[]",
            "{",
            r#"{"op":"x"}"#,
            r#"{"id":-1,"op":"x"}"#,
            r#"{"id":1.5,"op":"x"}"#,
            r#"{"id":1}"#,
            r#"{"id":1,"op":5}"#,
            r#"{"id":"1","op":"x"}"#,
        ] {
            let error = parse(text.as_bytes(), 1024).unwrap_err();
            assert_eq!(error.code, Code::Invalid, "{text}");
        }
        assert_eq!(
            parse(&vec![b'x'; 2000], 1024).unwrap_err().message,
            "request line is too long"
        );
        let request = parse(br#"{"id":1,"op":"x","n":-3,"f":1.5,"t":7}"#, 1024).unwrap();
        assert!(
            request.u64("n").is_err() && request.u64("f").is_err() && request.text("t").is_err()
        );
        assert!(request.optional_u64("n").is_err() && request.optional_text("t").is_err());
    }
    // Hex parameters are strict lowercase and bounded.
    #[test]
    fn hex_parameters_are_strict_and_bounded() {
        let request = parse(
            br#"{"id":1,"op":"x","a":"00ff","b":"00FF","c":"0","d":"aabbcc"}"#,
            1024,
        )
        .unwrap();
        assert_eq!(request.bytes("a", 2).unwrap(), vec![0, 255]);
        assert!(request.bytes("b", 2).is_err() && request.bytes("c", 2).is_err());
        assert!(request.bytes("d", 2).is_err());
        assert_eq!(unhex(&hex(&[1, 2, 254])).unwrap(), vec![1, 2, 254]);
    }
    // Replies are single newline-terminated JSON lines echoing the id.
    #[test]
    fn replies_are_single_lines() {
        let reply = ok(7, json!({"a": 1}));
        assert_eq!(reply.last(), Some(&b'\n'));
        assert_eq!(reply.iter().filter(|b| **b == b'\n').count(), 1);
        let value: Value = serde_json::from_slice(&reply).unwrap();
        assert_eq!(
            (value["id"].as_u64(), value["ok"].as_bool()),
            (Some(7), Some(true))
        );
        let failed: Value =
            serde_json::from_slice(&error(8, &ControlError::new(Code::UnknownOutcome, "m")))
                .unwrap();
        assert_eq!(failed["error"]["code"], "unknown_outcome");
        assert_eq!(failed["ok"], false);
    }
    // Line reading handles complete, split, closed and oversized input.
    #[test]
    fn line_reading_is_bounded() {
        let mut input: &[u8] = b"abc\nrest";
        assert_eq!(
            read_line(&mut input, 10, || true),
            Line::Complete(b"abc".to_vec())
        );
        let mut input: &[u8] = b"no newline";
        assert_eq!(read_line(&mut input, 100, || true), Line::Closed);
        let mut input: &[u8] = b"123456789\n";
        assert_eq!(read_line(&mut input, 3, || true), Line::TooLong);
    }
    // Every code has a distinct stable name.
    #[test]
    fn code_names_are_distinct() {
        let names: std::collections::BTreeSet<_> = [
            Code::Invalid,
            Code::Unsupported,
            Code::Denied,
            Code::Refused,
            Code::Insufficient,
            Code::Unavailable,
            Code::Limit,
            Code::UnknownOutcome,
            Code::Busy,
        ]
        .iter()
        .map(|c| c.name())
        .collect();
        assert_eq!(names.len(), 9);
    }
}
