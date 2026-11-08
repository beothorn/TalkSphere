//! Bounded logical validation; registered family semantics remain an independent check.
use super::{
    Error, schema,
    value::{Budget, Value},
};
use serde_json::Value as Schema;

/// Validate a logical value against a known schema and independent aggregate budgets.
/// Nonfinite permission is supplied only by trusted registered contract semantics.
/// # Errors
/// Invalid shape/types, unsupported descriptors and exhausted budgets are explicit failures.
pub fn validate(
    value: &Value,
    descriptor: &Schema,
    budget: Budget,
    nonfinite: bool,
) -> Result<(), Error> {
    tracing::trace!("Validating complete typed Services value");
    match schema::validate(descriptor).map_err(|e| match e {
        schema::Error::Invalid => Error::Invalid,
        schema::Error::Limit => Error::Limit,
    })? {
        schema::Support::Unknown => return Err(Error::Unsupported),
        schema::Support::Known => {}
    }
    value.bounded(budget)?;
    matches(value, descriptor, nonfinite)
}
// Compare exact logical kinds recursively after admission; unknown fields remain bounded inert data.
fn matches(value: &Value, schema: &Schema, nonfinite: bool) -> Result<(), Error> {
    let valid = match (schema["type"].as_str(), value) {
        (Some("bool"), Value::Bool(_))
        | (Some("int32"), Value::Int32(_))
        | (Some("int64"), Value::Int64(_))
        | (Some("uint32"), Value::Uint32(_))
        | (Some("uint64"), Value::Uint64(_))
        | (Some("string"), Value::String(_))
        | (Some("bytes"), Value::Bytes(_)) => true,
        (Some("float"), Value::Float(bits)) => nonfinite || f32::from_bits(*bits).is_finite(),
        (Some("double"), Value::Double(bits)) => nonfinite || f64::from_bits(*bits).is_finite(),
        (Some("array"), Value::Array(items)) => {
            for item in items {
                matches(item, &schema["items"], nonfinite)?;
            }
            true
        }
        (Some("object"), Value::Object(fields)) => {
            let descriptors = schema["fields"].as_object().ok_or(Error::Invalid)?;
            for (name, child) in descriptors {
                if let Some((_, value)) = fields.iter().find(|(key, _)| key == name) {
                    matches(value, child, nonfinite)?;
                } else if child.get("optional").and_then(Schema::as_bool) != Some(true) {
                    return Err(Error::Invalid);
                }
            }
            true
        }
        _ => false,
    };
    if valid { Ok(()) } else { Err(Error::Invalid) }
}

/// Locally installed contract rules, never loaded from a remote description.
pub trait Semantics: Send + Sync {
    /// Reject unsupported required extensions and validate operation-specific restrictions.
    /// # Errors
    /// Unsupported semantics and invalid combinations must fail without effects.
    fn validate(&self, descriptor: &Schema, value: &Value) -> Result<(), Error>;
    /// Whether this exact registered contract permits nonfinite logical values.
    fn nonfinite(&self) -> bool;
}
/// Validate shape and then mandatory registered semantics without executing work.
/// # Errors
/// Propagates schema, aggregate limit and family semantic failures before dispatch.
pub fn complete(
    value: &Value,
    descriptor: &Schema,
    budget: Budget,
    semantics: &dyn Semantics,
) -> Result<(), Error> {
    tracing::trace!("Checking logical shape and registered contract restrictions");
    validate(value, descriptor, budget, semantics.nonfinite())?;
    semantics.validate(descriptor, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    // Each primitive is exact; float exceptional values require trusted contract permission.
    #[test]
    fn vocabulary() {
        let b = Budget {
            depth: 4,
            nodes: 32,
            bytes: 128,
        };
        for (kind, value) in [
            ("bool", Value::Bool(false)),
            ("int32", Value::Int32(i32::MIN)),
            ("int64", Value::Int64(i64::MAX)),
            ("uint32", Value::Uint32(u32::MAX)),
            ("uint64", Value::Uint64(u64::MAX)),
            ("string", Value::String("€".into())),
            ("bytes", Value::Bytes(vec![])),
            ("float", Value::Float(0x80000000)),
            ("double", Value::Double(0)),
        ] {
            let s = serde_json::json!({"type":kind});
            assert_eq!(validate(&value, &s, b, false), Ok(()));
            assert_eq!(
                validate(&Value::Array(vec![]), &s, b, false),
                Err(Error::Invalid)
            );
        }
        for (kind, v) in [
            ("float", Value::Float(0x7f800000)),
            ("double", Value::Double(0x7ff8000000000001)),
        ] {
            let s = serde_json::json!({"type":kind});
            assert_eq!(validate(&v, &s, b, false), Err(Error::Invalid));
            assert_eq!(validate(&v, &s, b, true), Ok(()));
        }
    }
    // Required nested values are checked; optional omission and bounded unknown fields are permitted.
    #[test]
    fn structured() {
        let s = serde_json::json!({"type":"object","fields":{"x":{"type":"uint64","description":"Count."},"items":{"type":"array","items":{"type":"bytes"},"description":"Pieces."},"label":{"type":"string","description":"Label.","optional":true}}});
        let b = Budget {
            depth: 4,
            nodes: 32,
            bytes: 128,
        };
        let mut fields = vec![
            ("x".into(), Value::Uint64(1)),
            ("items".into(), Value::Array(vec![Value::Bytes(vec![])])),
        ];
        assert_eq!(
            validate(&Value::Object(fields.clone()), &s, b, false),
            Ok(())
        );
        fields.push(("extra".into(), Value::Bool(true)));
        assert_eq!(
            validate(&Value::Object(fields.clone()), &s, b, false),
            Ok(())
        );
        fields[0].1 = Value::Int32(1);
        assert_eq!(
            validate(&Value::Object(fields), &s, b, false),
            Err(Error::Invalid)
        );
        assert_eq!(
            validate(&Value::Object(vec![]), &s, b, false),
            Err(Error::Invalid)
        );
        assert_eq!(
            validate(
                &Value::Bool(true),
                &serde_json::json!({"type":"novel"}),
                b,
                false
            ),
            Err(Error::Unsupported)
        );
        assert_eq!(
            validate(&Value::Bool(true), &serde_json::json!({}), b, false),
            Err(Error::Invalid)
        );
    }
}
