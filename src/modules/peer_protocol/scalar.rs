//! Exact schema-directed scalar binding; detached values and containers are handled by the value layer.
use super::Error;
use crate::modules::services::value::Value;
use serde_json::{Value as Json, json};
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
/// Decode one scalar preserving logical width and IEEE bits under trusted nonfinite permission.
/// # Errors
/// Rejects incompatible values, noncanonical numeric forms and unsupported schema types.
pub fn decode(kind: &str, value: &Json, nonfinite: bool) -> Result<Value, Error> {
    tracing::trace!("Decoding exact peer scalar value");
    Ok(match kind {
        "bool" => Value::Bool(value.as_bool().ok_or(Error::Invalid)?),
        "string" => Value::String(value.as_str().ok_or(Error::Invalid)?.to_owned()),
        "uint32" => Value::Uint32(uint32(value)?),
        "int32" => {
            let n = value.as_number().ok_or(Error::Invalid)?;
            let s = n.to_string();
            if s.contains(['.', 'e', 'E']) {
                return Err(Error::Invalid);
            }
            Value::Int32(s.parse().map_err(|_| Error::Invalid)?)
        }
        "uint64" => Value::Uint64(uint64(value)?),
        "int64" => {
            let s = value.as_str().ok_or(Error::Invalid)?;
            let n: i64 = s.parse().map_err(|_| Error::Invalid)?;
            if n.to_string() != s {
                return Err(Error::Invalid);
            }
            Value::Int64(n)
        }
        "float" => {
            let bits = bits(value, "$float", 8)? as u32;
            let n = f32::from_bits(bits);
            if (!nonfinite && !n.is_finite()) || (n.is_nan() && bits != 0x7fc00000) {
                return Err(Error::Invalid);
            }
            Value::Float(bits)
        }
        "double" => {
            let bits = bits(value, "$double", 16)?;
            let n = f64::from_bits(bits);
            if (!nonfinite && !n.is_finite()) || (n.is_nan() && bits != 0x7ff8000000000000) {
                return Err(Error::Invalid);
            }
            Value::Double(bits)
        }
        _ => return Err(Error::Unsupported),
    })
}
/// Encode a scalar with exact width-dependent spelling and canonical NaNs.
/// # Errors
/// Unsupported containers/bytes and unauthorized exceptional floating values fail explicitly.
pub fn encode(value: &Value, nonfinite: bool) -> Result<Json, Error> {
    tracing::trace!("Encoding exact peer scalar value");
    Ok(match value {
        Value::Bool(v) => json!(v),
        Value::String(v) => json!(v),
        Value::Int32(v) => json!(v),
        Value::Uint32(v) => json!(v),
        Value::Int64(v) => json!(v.to_string()),
        Value::Uint64(v) => json!(v.to_string()),
        Value::Float(bits) => {
            let n = f32::from_bits(*bits);
            if !nonfinite && !n.is_finite() {
                return Err(Error::Invalid);
            }
            json!({"$float":format!("{:08x}",if n.is_nan(){0x7fc00000}else{*bits})})
        }
        Value::Double(bits) => {
            let n = f64::from_bits(*bits);
            if !nonfinite && !n.is_finite() {
                return Err(Error::Invalid);
            }
            json!({"$double":format!("{:016x}",if n.is_nan(){0x7ff8000000000000}else{*bits})})
        }
        _ => return Err(Error::Unsupported),
    })
}
// Require exactly one lowercase IEEE wrapper member with the specified fixed hexadecimal length.
fn bits(value: &Json, key: &str, length: usize) -> Result<u64, Error> {
    tracing::trace!("Validating canonical peer IEEE wrapper");
    let map = value.as_object().ok_or(Error::Invalid)?;
    if map.len() != 1 {
        return Err(Error::Invalid);
    }
    let s = map.get(key).and_then(Json::as_str).ok_or(Error::Invalid)?;
    if s.len() != length
        || !s
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(Error::Invalid);
    }
    u64::from_str_radix(s, 16).map_err(|_| Error::Invalid)
}
