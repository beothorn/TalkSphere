//! Peer metadata response sizing without implementing the Peer Protocol runtime.
use super::{Error, document::Document};

/// Fixed maximum JSON control body size, excluding the five-byte record header.
pub const CONTROL_BYTES: usize = 1_048_576;
/// Encode a complete Board or definition response using exact original document bytes.
/// # Errors
/// Returns Unsupported for unknown response kinds and Limit if the wrapper would exceed size/depth.
pub fn encode(kind: &str, document: &Document) -> Result<Vec<u8>, Error> {
    tracing::trace!("Sizing a complete peer metadata response");
    encode_source(kind, document.bytes())
}
/// Encode a complete Board response from an owner-issued immutable snapshot without reparsing its source.
/// # Errors
/// Limit rejects a wrapped response exceeding the fixed control maximum before allocation.
pub fn encode_snapshot(snapshot: &super::contract::Snapshot) -> Result<Vec<u8>, Error> {
    tracing::trace!("Encoding a pinned Board peer response");
    encode_source("board", snapshot.bytes())
}
// Both public inputs already own validated documents; preserve their exact source and check wrapper overhead first.
fn encode_source(kind: &str, bytes: &[u8]) -> Result<Vec<u8>, Error> {
    tracing::trace!("Checking and encoding exact metadata response source");
    let prefix: &[u8] = match kind {
        "board" => br#"{"type":"board","board":"#,
        "definition" => br#"{"type":"definition","definition":"#,
        _ => return Err(Error::Unsupported),
    };
    let length = prefix
        .len()
        .checked_add(bytes.len())
        .and_then(|n| n.checked_add(1))
        .ok_or(Error::Limit)?;
    if length > CONTROL_BYTES {
        return Err(Error::Limit);
    }
    // Document already has at most 31 containers; one wrapper preserves the peer depth limit of 32.
    let mut result = Vec::with_capacity(length);
    result.extend_from_slice(prefix);
    result.extend_from_slice(bytes);
    result.push(b'}');
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::board::config::Limits;
    // Exact body fit includes wrapper overhead, source escaping and all whitespace.
    #[test]
    fn exact_response_limits() {
        for kind in ["board", "definition"] {
            let d = Document::parse(b"{}", Limits::default()).unwrap();
            let overhead = encode(kind, &d).unwrap().len() - 2;
            let mut bytes = b"{}".to_vec();
            bytes.resize(CONTROL_BYTES - overhead, b' ');
            let d = Document::parse(&bytes, Limits::default()).unwrap();
            assert_eq!(encode(kind, &d).unwrap().len(), CONTROL_BYTES);
            bytes.push(b' ');
            let d = Document::parse(&bytes, Limits::default()).unwrap();
            assert!(matches!(encode(kind, &d), Err(Error::Limit)));
        }
        let d = Document::parse(r#"{"unknown":"é\\\""}"#.as_bytes(), Limits::default()).unwrap();
        assert!(serde_json::from_slice::<serde_json::Value>(&encode("board", &d).unwrap()).is_ok());
        assert!(matches!(encode("other", &d), Err(Error::Unsupported)));
    }
}
