//! Exact numeric field representations shared by peer control and service value binding.
use super::Error;
use serde_json::Value as Json;
/// Parse an exact canonical unsigned64 decimal string from a peer field.
/// # Errors
/// Rejects numbers, signs, leading zeros, fractions/exponents and overflow.
pub fn uint64(value: &Json) -> Result<u64, Error> {
    tracing::trace!("Parsing canonical peer uint64");
    let s = value.as_str().ok_or(Error::Invalid)?;
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) || (s.len() > 1 && s.starts_with('0'))
    {
        return Err(Error::Invalid);
    }
    s.parse().map_err(|_| Error::Invalid)
}
/// Parse a JSON uint32 integer token without decimal point or exponent.
/// # Errors
/// Invalid rejects wrong kinds, spelling and range.
pub fn uint32(value: &Json) -> Result<u32, Error> {
    tracing::trace!("Parsing exact peer uint32 token");
    let n = value.as_number().ok_or(Error::Invalid)?;
    let text = n.to_string();
    if !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::Invalid);
    }
    text.parse().map_err(|_| Error::Invalid)
}
