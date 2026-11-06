//! Services schema-format validation shared with Board; no handler runtime or value codec.
use crate::modules::board::{Error, offering::string};
use serde_json::Value;

/// Semantic support of a structurally valid schema description.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Support {
    /// Every described type is understood.
    Known,
    /// Unknown type remains discoverable but cannot be used automatically.
    Unknown,
}
/// Validate a primitive schema descriptor without interpreting metadata as executable rules.
/// # Errors
/// Rejects missing/non-string type and non-object descriptors; unknown types return Unknown.
pub fn validate(value: &Value) -> Result<Support, Error> {
    tracing::trace!("Validating a Services primitive schema description");
    if !value.is_object() {
        return Err(Error::Invalid);
    }
    Ok(match string(value, "type")? {
        "bool" | "int32" | "int64" | "uint32" | "uint64" | "float" | "double" | "string"
        | "bytes" => Support::Known,
        _ => Support::Unknown,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    // Every primitive is known; novel types remain inert, while malformed descriptors fail.
    #[test]
    fn primitive_vocabulary() {
        for t in [
            "bool", "int32", "int64", "uint32", "uint64", "float", "double", "string", "bytes",
        ] {
            assert_eq!(
                validate(&serde_json::json!({"type":t,"unknown":{"x":1}})),
                Ok(Support::Known)
            );
        }
        assert_eq!(
            validate(&serde_json::json!({"type":"org.example.unknown"})),
            Ok(Support::Unknown)
        );
        for v in [
            Value::Null,
            serde_json::json!({}),
            serde_json::json!({"type":3}),
            serde_json::json!({"type":""}),
        ] {
            assert_eq!(validate(&v), Err(Error::Invalid));
        }
    }
}
