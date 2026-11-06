//! Complete offering definition validation using shared Services schema formats.
use super::{
    Error,
    config::Limits,
    document::Document,
    offering::{Offering, integer, string},
};
use crate::modules::services::schema::{self, Support};

/// Exact immutable document plus whether all schema types are understood.
#[derive(Clone, Debug)]
pub struct Definition {
    /// Original complete definition bytes and logical tree.
    pub document: Document,
    /// Known only if input, output and mandatory check schemas are understood.
    pub support: Support,
}
impl Definition {
    /// Decode and bind a complete definition to one offering's exact identity fields.
    /// # Errors
    /// Rejects mismatches, missing schemas, economic terms and malformed nested descriptions.
    pub fn parse(bytes: &[u8], offering: &Offering, limits: Limits) -> Result<Self, Error> {
        tracing::trace!("Binding a complete service definition to its offering");
        let document = Document::parse(bytes, limits)?;
        let v = document.value();
        if string(v, "id")? != offering.id
            || string(v, "service")? != offering.service
            || string(v, "description")? != offering.description
            || v.get("serviceVersion").and_then(integer) != Some(offering.version as u64)
            || ["creditIssuer", "price", "pricing", "checkPrice", "info"]
                .iter()
                .any(|f| v.get(*f).is_some())
        {
            return Err(Error::Invalid);
        }
        let input = schema::validate(v.get("input").ok_or(Error::Invalid)?)?;
        let output = schema::validate(v.get("output").ok_or(Error::Invalid)?)?;
        let check = schema::check(v.get("check").ok_or(Error::Invalid)?)?;
        let support = if [input, output, check].contains(&Support::Unknown) {
            Support::Unknown
        } else {
            Support::Known
        };
        Ok(Self { document, support })
    }
    /// Compare the logical contract excluding offering-specific ID and conservatively comparing unknown metadata.
    /// Descriptions and all three schemas must agree; unknown details inside schemas are retained.
    pub fn same_contract(&self, other: &Self) -> bool {
        tracing::trace!("Comparing complete logical service definitions");
        let a = self.document.value();
        let b = other.document.value();
        if a.get("serviceVersion").and_then(integer) != b.get("serviceVersion").and_then(integer) {
            return false;
        }
        let equivalent = |left: &serde_json::Value, right: &serde_json::Value| {
            left.as_object().is_some_and(|o| {
                o.iter()
                    .filter(|(key, _)| !matches!(key.as_str(), "id" | "serviceVersion"))
                    .all(|(key, value)| right.get(key) == Some(value))
            })
        };
        equivalent(a, b) && equivalent(b, a)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::board::{offering::Board, test_support};
    // Each mismatch and missing schema fails independently; a complete literal fixture succeeds.
    #[test]
    fn definitions_match_exact_fields() {
        let b = Board::parse(test_support::board(false, 7).as_bytes(), Limits::default()).unwrap();
        let o = &b.offerings["fixture"];
        let base: serde_json::Value = serde_json::from_str(test_support::definition()).unwrap();
        let d =
            Definition::parse(test_support::definition().as_bytes(), o, Limits::default()).unwrap();
        assert_eq!(d.support, Support::Known);
        assert!(d.same_contract(&d));
        for field in [
            "id",
            "service",
            "serviceVersion",
            "description",
            "input",
            "output",
            "check",
        ] {
            let mut bad = base.clone();
            bad.as_object_mut().unwrap().remove(field);
            assert!(Definition::parse(bad.to_string().as_bytes(), o, Limits::default()).is_err());
        }
        for field in ["id", "service", "description"] {
            let mut bad = base.clone();
            bad[field] = "different".into();
            assert!(Definition::parse(bad.to_string().as_bytes(), o, Limits::default()).is_err());
        }
        let mut bad = base.clone();
        bad["price"] = 7.into();
        assert!(Definition::parse(bad.to_string().as_bytes(), o, Limits::default()).is_err());
        bad = base;
        bad["input"]["type"] = "unknown.type".into();
        let unknown = Definition::parse(bad.to_string().as_bytes(), o, Limits::default()).unwrap();
        assert_eq!(unknown.support, Support::Unknown);
        assert!(!d.same_contract(&unknown));
    }
}

#[cfg(test)]
mod comparison_tests {
    use super::*;
    // Numeric version spelling does not change a contract; unfamiliar metadata is compared conservatively.
    #[test]
    fn exact_versions_and_unknown_metadata() {
        let b = super::super::offering::Board::parse(
            super::super::test_support::board(false, 7).as_bytes(),
            Limits::default(),
        )
        .unwrap();
        let o = &b.offerings["fixture"];
        let d = Definition::parse(
            super::super::test_support::definition().as_bytes(),
            o,
            Limits::default(),
        )
        .unwrap();
        let mut v = d.document.value().clone();
        v["serviceVersion"] = serde_json::from_str("1.0").unwrap();
        let same = Definition::parse(v.to_string().as_bytes(), o, Limits::default()).unwrap();
        assert!(d.same_contract(&same));
        v["newContractMetadata"] = "unfamiliar rule".into();
        let different = Definition::parse(v.to_string().as_bytes(), o, Limits::default()).unwrap();
        assert!(!d.same_contract(&different));
    }
}
