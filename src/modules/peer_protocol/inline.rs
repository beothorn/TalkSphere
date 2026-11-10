//! Canonical inline byte values, separate from fixed control identifiers and detached transfer ownership.
use super::{Error, base64};
use serde_json::{Value, json};
/// Maximum decoded inline bytes in wire version one.
pub const INLINE_MAX: usize = 4096;
/// Encode a byte value inline only when it fits the mandatory threshold.
/// # Errors
/// Limit requires the caller to use a detached body for larger values.
pub fn encode(bytes: &[u8]) -> Result<Value, Error> {
    tracing::trace!("Encoding bounded inline service bytes");
    if bytes.len() > INLINE_MAX {
        return Err(Error::Limit);
    }
    Ok(json!({"$base64":base64::encode(bytes)}))
}
/// Decode exactly one canonical inline byte wrapper.
/// # Errors
/// Rejects unknown descriptor members, wrong kind/padding and excessive decoded length.
pub fn decode(value: &Value) -> Result<Vec<u8>, Error> {
    tracing::trace!("Decoding canonical inline service bytes");
    let map = value.as_object().ok_or(Error::Invalid)?;
    if map.len() != 1 {
        return Err(Error::Invalid);
    }
    base64::decode(
        map.get("$base64")
            .and_then(Value::as_str)
            .ok_or(Error::Invalid)?,
        INLINE_MAX,
    )
}
