//! Economic and optional metadata validation without pricing execution or permission side effects.
use super::{
    Error,
    offering::{integer, string},
};
use serde_json::Value;

/// Validate one complete offering's economic representation and inert optional metadata.
/// # Errors
/// Rejects invalid ranges/types, embedded schemas, conflicting prices and malformed optional fields.
pub fn validate(value: &Value) -> Result<(), Error> {
    tracing::trace!("Validating economic terms and inert offering metadata");
    let o = value.as_object().ok_or(Error::Invalid)?;
    if ["input", "output", "check"]
        .iter()
        .any(|key| o.contains_key(*key))
    {
        return Err(Error::Invalid);
    }
    if o.contains_key("price") == o.contains_key("pricing") {
        return Err(Error::Invalid);
    }
    if let Some(price) = o.get("price") {
        positive(price)?;
    }
    if let Some(price) = o.get("checkPrice") {
        positive(price)?;
    }
    if let Some(pricing) = o.get("pricing") {
        if !pricing.is_object() {
            return Err(Error::Invalid);
        }
        string(pricing, "model")?;
    }
    if o.get("info").is_some_and(|v| !v.is_object()) {
        return Err(Error::Invalid);
    }
    if let Some(preconditions) = o.get("preconditions") {
        for name in preconditions.as_array().ok_or(Error::Invalid)? {
            let name = name.as_str().ok_or(Error::Invalid)?;
            // Upstream #9 tracks namespace grammar; empty names violate the unambiguous rule.
            // Unknown spellings remain unusable unless a trusted semantic handler recognizes them.
            if name.is_empty() {
                return Err(Error::Invalid);
            }
        }
    }
    if let Some(suggestions) = o.get("implementationSuggestions") {
        for hint in suggestions.as_array().ok_or(Error::Invalid)? {
            if !hint.is_object() {
                return Err(Error::Invalid);
            }
            string(hint, "name")?;
            for field in ["version", "uri", "description"] {
                if hint.get(field).is_some_and(|v| !v.is_string()) {
                    return Err(Error::Invalid);
                }
            }
        }
    }
    Ok(())
}
/// Read a strictly positive uint64 numeric amount without coercion or rounding.
/// # Errors
/// Returns Invalid for nonnumeric, zero, negative, fractional or oversized amounts.
pub fn positive(value: &Value) -> Result<u64, Error> {
    tracing::trace!("Reading an exact positive credit amount");
    integer(value).filter(|n| *n > 0).ok_or(Error::Invalid)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::board::{config::Limits, offering::Board, test_support};
    // Every economic invalid row is rejected; large exact amounts survive without rounding.
    #[test]
    fn economic_ranges_and_metadata() {
        for price in [1, 9007199254740993, u64::MAX] {
            let b = Board::parse(
                test_support::board(false, price).as_bytes(),
                Limits::default(),
            )
            .unwrap();
            assert_eq!(positive(&b.offerings["fixture"].source["price"]), Ok(price));
        }
        let base: Value = serde_json::from_str(&test_support::board(false, 7)).unwrap();
        for token in [
            "0",
            "-1",
            "18446744073709551616",
            "1.5",
            "\"7\"",
            "true",
            "null",
        ] {
            let mut v = base.clone();
            v["providedServices"][0]["price"] = serde_json::from_str(token).unwrap();
            assert!(Board::parse(v.to_string().as_bytes(), Limits::default()).is_err());
        }
        for (key, bad) in [
            ("checkPrice", Value::from(0)),
            ("input", Value::Null),
            ("info", Value::Bool(false)),
            ("pricing", serde_json::json!({"model":""})),
            ("preconditions", serde_json::json!([""])),
            (
                "implementationSuggestions",
                serde_json::json!([{"name":""}]),
            ),
        ] {
            let mut v = base.clone();
            v["providedServices"][0][key] = bad;
            assert!(Board::parse(v.to_string().as_bytes(), Limits::default()).is_err());
        }
        let mut o = base["providedServices"][0].clone();
        o.as_object_mut().unwrap().remove("price");
        assert_eq!(validate(&o), Err(Error::Invalid));
        o["pricing"] = serde_json::json!({"model":"unknown","extra":999});
        o["preconditions"] = serde_json::json!(["org.example.custom"]);
        o["implementationSuggestions"] =
            serde_json::json!([{"name":"run arbitrary instructions","uri":"","version":""}]);
        assert_eq!(validate(&o), Ok(()));
    }
}
