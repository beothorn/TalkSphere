//! Bounded lossless JSON documents for Board and Services format validation.
use super::{Error, config::Limits};
use serde_json::Value;
use std::{collections::BTreeSet, sync::Arc};

/// Validated syntax with exact source bytes and arbitrary-precision numeric tokens.
#[derive(Clone, Debug)]
pub struct Document {
    source: Arc<[u8]>,
    value: Value,
}
impl Document {
    /// Decode one complete UTF-8 JSON value after bounded structural preflight.
    /// # Errors
    /// Rejects malformed/trailing input, duplicate decoded keys and every configured budget breach.
    pub fn parse(bytes: &[u8], limits: Limits) -> Result<Self, Error> {
        tracing::trace!("Decoding a bounded lossless document");
        limits.validate()?;
        if bytes.len() > limits.document_bytes {
            return Err(Error::Limit);
        }
        std::str::from_utf8(bytes).map_err(|_| Error::Invalid)?;
        let mut parser = Scan {
            bytes,
            pos: 0,
            nodes: 0,
            limits,
        };
        parser.value(0)?;
        parser.space();
        if parser.pos != bytes.len() {
            return Err(Error::Invalid);
        }
        let value = serde_json::from_slice(bytes).map_err(|_| Error::Invalid)?;
        Ok(Self {
            source: Arc::from(bytes),
            value,
        })
    }
    /// Borrow exact original bytes, retaining unknown fields and numeric spelling when forwarding.
    pub fn bytes(&self) -> &[u8] {
        tracing::trace!("Borrowing original document bytes");
        &self.source
    }
    /// Borrow the immutable logical JSON tree; numeric values never pass through floating point.
    pub fn value(&self) -> &Value {
        tracing::trace!("Borrowing lossless document tree");
        &self.value
    }
}
struct Scan<'a> {
    bytes: &'a [u8],
    pos: usize,
    nodes: usize,
    limits: Limits,
}
impl Scan<'_> {
    // Consume only JSON whitespace; all other bytes remain part of validation.
    fn space(&mut self) {
        let _span = tracing::trace_span!("bounded_json_preflight").entered();
        while self
            .bytes
            .get(self.pos)
            .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
        {
            self.pos += 1;
        }
    }
    // Count before processing a value or member so parser work remains bounded.
    fn node(&mut self) -> Result<(), Error> {
        let _span = tracing::trace_span!("bounded_json_preflight").entered();
        self.nodes += 1;
        if self.nodes > self.limits.nodes {
            Err(Error::Limit)
        } else {
            Ok(())
        }
    }
    // Decode one bounded JSON string with the library's Unicode/escape validation.
    fn string(&mut self) -> Result<String, Error> {
        let _span = tracing::trace_span!("bounded_json_preflight").entered();
        let start = self.pos;
        self.pos += 1;
        loop {
            match self.bytes.get(self.pos) {
                Some(b'"') => {
                    self.pos += 1;
                    break;
                }
                Some(b'\\') => self.pos += 2,
                Some(_) => self.pos += 1,
                None => return Err(Error::Invalid),
            }
        }
        let text: String =
            serde_json::from_slice(&self.bytes[start..self.pos]).map_err(|_| Error::Invalid)?;
        if text.len() > self.limits.string_bytes {
            return Err(Error::Limit);
        }
        Ok(text)
    }
    // Validate container nesting and duplicate decoded keys before building the complete tree.
    fn value(&mut self, depth: usize) -> Result<(), Error> {
        let _span = tracing::trace_span!("bounded_json_preflight").entered();
        self.space();
        self.node()?;
        match self.bytes.get(self.pos).copied().ok_or(Error::Invalid)? {
            b'{' | b'[' => {
                if depth >= self.limits.depth {
                    return Err(Error::Limit);
                }
                let object = self.bytes[self.pos] == b'{';
                let end = if object { b'}' } else { b']' };
                self.pos += 1;
                self.space();
                if self.bytes.get(self.pos) == Some(&end) {
                    self.pos += 1;
                    return Ok(());
                }
                let mut keys = BTreeSet::new();
                let mut count = 0;
                loop {
                    if object {
                        self.space();
                        self.node()?;
                        if self.bytes.get(self.pos) != Some(&b'"') {
                            return Err(Error::Invalid);
                        }
                        if !keys.insert(self.string()?) {
                            return Err(Error::Invalid);
                        }
                        self.space();
                        if self.bytes.get(self.pos) != Some(&b':') {
                            return Err(Error::Invalid);
                        }
                        self.pos += 1;
                    } else {
                        count += 1;
                        if count > self.limits.array_elements {
                            return Err(Error::Limit);
                        }
                    }
                    self.value(depth + 1)?;
                    self.space();
                    match self.bytes.get(self.pos) {
                        Some(b) if *b == end => {
                            self.pos += 1;
                            break;
                        }
                        Some(b',') => self.pos += 1,
                        _ => return Err(Error::Invalid),
                    }
                }
            }
            b'"' => {
                self.string()?;
            }
            _ => {
                let start = self.pos;
                while self.bytes.get(self.pos).is_some_and(|b| {
                    !matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b',' | b']' | b'}')
                }) {
                    self.pos += 1;
                }
                if start == self.pos {
                    return Err(Error::Invalid);
                }
                serde_json::from_slice::<Value>(&self.bytes[start..self.pos])
                    .map_err(|_| Error::Invalid)?;
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    // Source preservation keeps nested unknown metadata and numeric extremes exact.
    #[test]
    fn exact_source_and_invalid_syntax() {
        let bytes=br#"{"unknown":{"x":9007199254740993,"y":18446744073709551615,"z":18446744073709551616}}"#;
        let d = Document::parse(bytes, Limits::default()).unwrap();
        assert_eq!(d.bytes(), bytes);
        assert_eq!(d.value()["unknown"]["y"].as_u64(), Some(u64::MAX));
        for bad in [
            &b"{\"x\":1,\"x\":2}"[..],
            br#"{"a":0,"\u0061":1}"#,
            b"[] []",
            b"[1,]",
            b"{",
            b"\xff",
            b"01",
            b"\"\\uD800\"",
            b"truex",
            br#"{"a":{"x":1,"x":2}}"#,
        ] {
            assert!(Document::parse(bad, Limits::default()).is_err(), "{bad:?}");
        }
    }
    // Each budget is enforced before complete-tree allocation, including exact fit.
    #[test]
    fn parsing_budgets() {
        let base = Limits::default();
        for (bytes, limits) in [
            (
                b"[] ".as_slice(),
                Limits {
                    document_bytes: 2,
                    string_bytes: 2,
                    ..base
                },
            ),
            (b"[[0]]", Limits { depth: 1, ..base }),
            (b"[0,1]", Limits { nodes: 2, ..base }),
            (
                b"[0,1]",
                Limits {
                    array_elements: 1,
                    ..base
                },
            ),
            (
                br#""abc""#,
                Limits {
                    string_bytes: 2,
                    ..base
                },
            ),
        ] {
            assert!(matches!(Document::parse(bytes, limits), Err(Error::Limit)));
        }
        assert!(
            Document::parse(
                b"[]",
                Limits {
                    document_bytes: 2,
                    string_bytes: 2,
                    ..base
                }
            )
            .is_ok()
        );
        assert!(
            Document::parse(
                b"[0]",
                Limits {
                    array_elements: 1,
                    ..base
                }
            )
            .is_ok()
        );
        assert!(Document::parse(br#""a\\b""#, base).is_ok());
    }
}
