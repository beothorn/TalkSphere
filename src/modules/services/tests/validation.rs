//! Mutation evidence for independent shape and mandatory contract validation.
use super::{
    Error,
    validation::{self, Semantics},
    value::{Budget, Value},
};
use serde_json::json;
struct Restricted;
impl Semantics for Restricted {
    // Only the exact operation's known field is allowed; metadata cannot opt into new semantics.
    fn validate(&self, schema: &serde_json::Value, value: &Value) -> Result<(), Error> {
        if schema.get("requiredExtension").is_some() {
            return Err(Error::Unsupported);
        }
        if let Value::Object(fields) = value
            && fields.iter().any(|(name, _)| name != "x")
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
    // No nonfinite value has meaning in this fixture's contract.
    fn nonfinite(&self) -> bool {
        false
    }
}
// Optional unknown fields do not bypass a contract's explicit discriminator restrictions.
#[test]
fn mandatory_semantics() {
    let schema =
        json!({"type":"object","fields":{"x":{"type":"uint64","description":"Exact count."}}});
    let b = Budget {
        depth: 4,
        nodes: 32,
        bytes: 64,
    };
    let v = Value::Object(vec![
        ("x".into(), Value::Uint64(1)),
        ("forbidden".into(), Value::Bool(false)),
    ]);
    assert_eq!(validation::validate(&v, &schema, b, false), Ok(()));
    assert_eq!(
        validation::complete(&v, &schema, b, &Restricted),
        Err(Error::Invalid)
    );
    let mut schema = schema;
    schema["requiredExtension"] = true.into();
    assert_eq!(
        validation::complete(
            &Value::Object(vec![("x".into(), Value::Uint64(1))]),
            &schema,
            b,
            &Restricted
        ),
        Err(Error::Unsupported)
    );
}
// Malformed nested descriptors, optional null stand-ins and wrong items cannot pass shape validation.
#[test]
fn structural_mutations() {
    let b = Budget {
        depth: 4,
        nodes: 32,
        bytes: 64,
    };
    for schema in [
        json!({"type":"object","fields":{"x":{"type":"string"}}}),
        json!({"type":"array"}),
        json!({"type":"string","optional":true}),
        json!({"type":"array","items":{"type":"string","optional":true}}),
    ] {
        assert_eq!(
            validation::validate(&Value::Bool(false), &schema, b, false),
            Err(Error::Invalid)
        );
    }
    let schema = json!({"type":"object","fields":{"label":{"type":"string","description":"Label.","optional":true}}});
    assert_eq!(
        validation::validate(&Value::Object(vec![]), &schema, b, false),
        Ok(())
    );
    assert_eq!(
        validation::validate(
            &Value::Object(vec![("label".into(), Value::Array(vec![]))]),
            &schema,
            b,
            false
        ),
        Err(Error::Invalid)
    );
    assert_eq!(
        validation::validate(
            &Value::Array(vec![Value::Uint64(1)]),
            &json!({"type":"array","items":{"type":"bytes"}}),
            b,
            false
        ),
        Err(Error::Invalid)
    );
    assert_eq!(
        validation::validate(
            &Value::Bool(false),
            &json!({"type":"bool"}),
            Budget { nodes: 0, ..b },
            false
        ),
        Err(Error::Limit)
    );
}

// Both endpoints, exact zero signs and finite extrema stay lossless; wider or signed stand-ins cannot cross integer contracts.
#[test]
fn literal_type_boundaries() {
    let b = Budget {
        depth: 31,
        nodes: 64,
        bytes: 4096,
    };
    for (kind, values) in [
        ("bool", vec![Value::Bool(true), Value::Bool(false)]),
        (
            "int32",
            vec![Value::Int32(i32::MIN), Value::Int32(i32::MAX)],
        ),
        (
            "int64",
            vec![Value::Int64(i64::MIN), Value::Int64(i64::MAX)],
        ),
        ("uint32", vec![Value::Uint32(0), Value::Uint32(u32::MAX)]),
        ("uint64", vec![Value::Uint64(0), Value::Uint64(u64::MAX)]),
        (
            "string",
            vec![Value::String(String::new()), Value::String("€".into())],
        ),
        (
            "bytes",
            vec![Value::Bytes(vec![]), Value::Bytes(vec![0, 255])],
        ),
        (
            "float",
            vec![
                Value::Float(0),
                Value::Float(0x80000000),
                Value::Float(0x7f7fffff),
            ],
        ),
        (
            "double",
            vec![
                Value::Double(0),
                Value::Double(0x8000000000000000),
                Value::Double(0x7fefffffffffffff),
            ],
        ),
    ] {
        let descriptor = serde_json::json!({"type":kind});
        for value in values {
            assert_eq!(validation::validate(&value, &descriptor, b, false), Ok(()));
        }
    }
    for (kind, invalid) in [
        ("int32", Value::Int64(-2147483649)),
        ("int32", Value::Int64(2147483648)),
        ("uint32", Value::Int32(-1)),
        ("uint32", Value::Uint64(4294967296)),
        ("uint64", Value::Int64(-1)),
        ("int64", Value::Uint64(9223372036854775808)),
        ("bool", Value::Int32(0)),
        ("bool", Value::String("true".into())),
    ] {
        assert_eq!(
            validation::validate(&invalid, &serde_json::json!({"type":kind}), b, false),
            Err(Error::Invalid)
        );
    }
}
