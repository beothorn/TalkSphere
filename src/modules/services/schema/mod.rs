//! Services schema-format validation shared with Board; no handler runtime or value codec.

use serde_json::Value;

/// Schema format failures, independent of any consuming module's error representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// A required descriptor field is malformed or missing.
    Invalid,
    /// The independent recursion or node budget was exceeded.
    Limit,
}
// Require a nonempty schema string without assigning executable meaning to descriptive text.
fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str, Error> {
    tracing::trace!("Reading a required Services schema string");
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or(Error::Invalid)
}
/// Semantic support of a structurally valid schema description.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Support {
    /// Every described type is understood.
    Known,
    /// Unknown type remains discoverable but cannot be used automatically.
    Unknown,
}
/// Validate a bounded complete schema descriptor without interpreting metadata as executable rules.
/// # Errors
/// Rejects missing/non-string type and non-object descriptors; unknown types return Unknown.
pub fn validate(value: &Value) -> Result<Support, Error> {
    tracing::trace!("Validating bounded Services schema structure");
    walk(value, false, 0, &mut 65536)
}

// Bound recursion independently of callers and propagate unsupported nested type semantics.
fn walk(value: &Value, field: bool, depth: usize, remaining: &mut usize) -> Result<Support, Error> {
    tracing::trace!("Checking nested schema structure");
    if depth > 31 || *remaining == 0 {
        return Err(Error::Limit);
    }
    *remaining -= 1;
    if !value.is_object() {
        return Err(Error::Invalid);
    }
    if let Some(optional) = value.get("optional")
        && (!field || optional.as_bool().is_none())
    {
        return Err(Error::Invalid);
    }
    if field {
        string(value, "description")?;
    }
    let mut support = Support::Known;
    match string(value, "type")? {
        "object" => {
            for (name, child) in value
                .get("fields")
                .and_then(Value::as_object)
                .ok_or(Error::Invalid)?
            {
                if name.is_empty() {
                    return Err(Error::Invalid);
                }
                if walk(child, true, depth + 1, remaining)? == Support::Unknown {
                    support = Support::Unknown;
                }
            }
        }
        "array" => {
            support = walk(
                value.get("items").ok_or(Error::Invalid)?,
                false,
                depth + 1,
                remaining,
            )?
        }
        "bool" | "int32" | "int64" | "uint32" | "uint64" | "float" | "double" | "string"
        | "bytes" => {}
        _ => support = Support::Unknown,
    }
    Ok(support)
}
/// Validate mandatory check procedure description and both check schemas.
/// # Errors
/// Rejects missing/empty description, malformed schemas and exceeded recursion budgets.
pub fn check(value: &Value) -> Result<Support, Error> {
    tracing::trace!("Validating a complete check definition");
    string(value, "description")?;
    let input = validate(value.get("input").ok_or(Error::Invalid)?)?;
    let output = validate(value.get("output").ok_or(Error::Invalid)?)?;
    Ok(if input == Support::Known && output == Support::Known {
        Support::Known
    } else {
        Support::Unknown
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
    // Nested named fields need descriptions; optionality belongs only to object fields.
    #[test]
    fn composite_and_check_rules() {
        let good = serde_json::json!({"type":"object","fields":{"label":{"type":"array","description":"Labels.","items":{"type":"string"},"optional":true}}});
        assert_eq!(validate(&good), Ok(Support::Known));
        let mut bad = good.clone();
        bad["fields"]["label"]
            .as_object_mut()
            .unwrap()
            .remove("description");
        assert_eq!(validate(&bad), Err(Error::Invalid));
        bad = good.clone();
        bad["fields"]["label"]["items"]["optional"] = true.into();
        assert_eq!(validate(&bad), Err(Error::Invalid));
        bad = good.clone();
        bad["fields"]["label"]["items"]["type"] = "new.type".into();
        assert_eq!(validate(&bad), Ok(Support::Unknown));
        let c = serde_json::json!({"description":"Compare labels.","input":good,"output":{"type":"bool"}});
        assert_eq!(check(&c), Ok(Support::Known));
        for field in ["description", "input", "output"] {
            let mut b = c.clone();
            b.as_object_mut().unwrap().remove(field);
            assert!(check(&b).is_err());
        }
        let mut deep = serde_json::json!({"type":"string"});
        for _ in 0..33 {
            deep = serde_json::json!({"type":"array","items":deep});
        }
        assert_eq!(validate(&deep), Err(Error::Limit));
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;
    // The reusable format validator has a node bound even when called without Board's JSON decoder.
    #[test]
    fn independent_schema_node_budget() {
        let field = serde_json::json!({"type":"string","description":"One bounded field."});
        let fields: serde_json::Map<String, Value> = (0..65536)
            .map(|i| (format!("f{i}"), field.clone()))
            .collect();
        assert_eq!(
            validate(&serde_json::json!({"type":"object","fields":fields})),
            Err(Error::Limit)
        );
    }
}
