//! Structural Board and offering identity validation, independent of local service support.
use super::{Error, config::Limits, document::Document};
use crate::modules::identity::{api::IdentityId, text};
use serde_json::Value;
use std::collections::BTreeMap;

/// Publisher role relative to the work, independent of connection direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Publisher performs the work.
    Provided,
    /// Publisher requests and pays for the work.
    Wanted,
}
/// One structurally validated offering and preserved metadata.
#[derive(Clone, Debug)]
pub struct Offering {
    /// Unique Board-local lookup key.
    pub id: String,
    /// Exact contract identifier.
    pub service: String,
    /// Exact nonzero contract version.
    pub version: u32,
    /// Observable operation description, never executable instructions.
    pub description: String,
    /// Explicit issuer of payment credits.
    pub issuer: IdentityId,
    /// Publisher's provided/wanted role.
    pub direction: Direction,
    /// Exact complete JSON object, including unknown metadata.
    pub source: Value,
}
/// Complete syntax/identity-validated Board, indexed without array priority.
#[derive(Clone, Debug)]
pub struct Board {
    /// Original bounded document.
    pub document: Document,
    /// Unique offering lookup shared across both directions.
    pub offerings: BTreeMap<String, Offering>,
}
impl Board {
    /// Validate version, required lists and exact offering identities without consulting a runtime.
    /// # Errors
    /// Rejects malformed fields, unsupported Board versions, duplicate IDs and configured limits.
    pub fn parse(bytes: &[u8], limits: Limits) -> Result<Self, Error> {
        tracing::trace!("Validating Board document and offering identities");
        let document = Document::parse(bytes, limits)?;
        let root = document.value().as_object().ok_or(Error::Invalid)?;
        if integer(root.get("version").ok_or(Error::Invalid)?).ok_or(Error::Invalid)? != 1 {
            return Err(Error::Unsupported);
        }
        let mut offerings = BTreeMap::new();
        for (field, direction) in [
            ("providedServices", Direction::Provided),
            ("wantedServices", Direction::Wanted),
        ] {
            for value in root
                .get(field)
                .and_then(Value::as_array)
                .ok_or(Error::Invalid)?
            {
                let o = value.as_object().ok_or(Error::Invalid)?;
                let version = integer(o.get("serviceVersion").ok_or(Error::Invalid)?)
                    .filter(|n| *n > 0 && *n <= u32::MAX as u64)
                    .ok_or(Error::Invalid)? as u32;
                let offering = Offering {
                    id: string(value, "id")?.into(),
                    service: string(value, "service")?.into(),
                    version,
                    description: string(value, "description")?.into(),
                    issuer: text::parse(string(value, "creditIssuer")?)
                        .map_err(|_| Error::Invalid)?,
                    direction,
                    source: value.clone(),
                };
                if offerings.insert(offering.id.clone(), offering).is_some() {
                    return Err(Error::Invalid);
                }
                if offerings.len() > limits.definitions {
                    return Err(Error::Limit);
                }
            }
        }
        Ok(Self {
            document,
            offerings,
        })
    }
}
/// Read a nonempty string without trimming or interpreting descriptive content.
/// # Errors
/// Returns Invalid for missing, empty or non-string fields.
pub fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str, Error> {
    tracing::trace!("Reading a required document string");
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or(Error::Invalid)
}
/// Read an unsigned integer token without floating-point rounding or coercion.
pub fn integer(value: &Value) -> Option<u64> {
    tracing::trace!("Reading an exact unsigned JSON integer");
    value.as_number().and_then(|n| n.as_u64())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::board::test_support;
    // Both directions and distinct IDs work; all identity/type/version and duplicate placements fail.
    #[test]
    fn identity_validation() {
        for wanted in [false, true] {
            let b =
                Board::parse(test_support::board(wanted, 7).as_bytes(), Limits::default()).unwrap();
            assert_eq!(
                b.offerings["fixture"].direction,
                if wanted {
                    Direction::Wanted
                } else {
                    Direction::Provided
                }
            );
        }
        let base: Value = serde_json::from_str(&test_support::board(false, 7)).unwrap();
        for field in [
            "id",
            "service",
            "description",
            "creditIssuer",
            "serviceVersion",
        ] {
            for bad in [Value::Null, Value::Bool(true), Value::String(String::new())] {
                let mut v = base.clone();
                v["providedServices"][0][field] = bad;
                assert!(Board::parse(v.to_string().as_bytes(), Limits::default()).is_err());
            }
        }
        for field in ["version", "providedServices", "wantedServices"] {
            let mut v = base.clone();
            v.as_object_mut().unwrap().remove(field);
            assert!(Board::parse(v.to_string().as_bytes(), Limits::default()).is_err());
        }
        for field in ["providedServices", "wantedServices"] {
            let mut v = base.clone();
            let o = v["providedServices"][0].clone();
            v[field].as_array_mut().unwrap().push(o);
            assert!(Board::parse(v.to_string().as_bytes(), Limits::default()).is_err());
        }
        let mut v = base;
        v["version"] = 2.into();
        assert!(matches!(
            Board::parse(v.to_string().as_bytes(), Limits::default()),
            Err(Error::Unsupported)
        ));
        assert!(
            Board::parse(
                br#"{"version":1,"providedServices":[],"wantedServices":[]}"#,
                Limits::default()
            )
            .is_ok()
        );
    }
}
