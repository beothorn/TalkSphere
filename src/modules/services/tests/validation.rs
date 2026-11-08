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
