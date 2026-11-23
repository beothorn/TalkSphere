//! Validation of a Page offering into immutable terms: one fixed price and two limits.
//!
//! Callers: the provider adapter, the requester client and local publication. A version 1 Page offering uses the Board's fixed `price`, never a
//! `pricing` object, carries the `cr2se.identity` precondition and the two `info` limits, and never a `checkPrice` because the Page check is
//! local and included in each successful invocation. The credit issuer need not be the publisher.
use crate::modules::{
    board::offering::{Offering, integer},
    pages::error::Error,
};
use serde_json::Value;

/// Service identifier of the standard offering.
pub const SERVICE: &str = "cr2se.page";
/// The only supported service version.
pub const VERSION: u32 = 1;
/// Largest allowed `maximumPathBytes`.
pub const MAXIMUM_PATH_BYTES_LIMIT: u64 = 65_535;

/// Validated Page version-one terms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Terms {
    /// The exact price of one successful `get`, in credits of the offering's issuer.
    pub price: u64,
    /// Largest canonical path, in UTF-8 bytes, this offering accepts.
    pub maximum_path_bytes: u64,
    /// Largest content size, in bytes, this offering returns.
    pub maximum_resource_bytes: u64,
}

impl Terms {
    /// Validate a provided offering.
    /// # Errors
    /// `Unsupported` for another service or version; `Invalid` for a missing, zero, fractional or out-of-range value, a `pricing` object, a `checkPrice`, a
    /// missing `cr2se.identity` precondition or a wanted offering.
    pub fn parse(offering: &Offering) -> Result<Self, Error> {
        tracing::trace!("Validating a Page offering");
        let v = &offering.source;
        if offering.service != SERVICE || offering.version != VERSION {
            return Err(Error::Unsupported);
        }
        if offering.direction != crate::modules::board::offering::Direction::Provided
            || v.get("pricing").is_some()
            || v.get("checkPrice").is_some()
            || !v["preconditions"]
                .as_array()
                .is_some_and(|a| a.iter().any(|x| x == "cr2se.identity"))
        {
            return Err(Error::Invalid);
        }
        let info = &v["info"];
        let maximum_path_bytes = positive(info, "maximumPathBytes")?;
        if maximum_path_bytes > MAXIMUM_PATH_BYTES_LIMIT {
            return Err(Error::Invalid);
        }
        Ok(Self {
            price: positive(v, "price")?,
            maximum_path_bytes,
            maximum_resource_bytes: positive(info, "maximumResourceBytes")?,
        })
    }
}

// An exact positive uint64 member, rejecting decimals, negatives, strings and zero.
fn positive(value: &Value, name: &str) -> Result<u64, Error> {
    tracing::trace!("Reading a positive Page term");
    value
        .get(name)
        .and_then(integer)
        .filter(|n| *n > 0)
        .ok_or(Error::Invalid)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::modules::{
        board::{config::Limits, offering::Board},
        identity::api::IdentityId,
    };

    /// The acceptance offering issued by identity `[2; 32]`: price 4, paths up to 1024 bytes and resources up to 100 bytes.
    pub(crate) fn offering() -> Offering {
        let mut o: Value = serde_json::from_str(include_str!("fixtures/offering.json")).unwrap();
        o["creditIssuer"] = crate::modules::identity::text::format(IdentityId([2; 32])).into();
        let board = serde_json::json!({"version":1,"providedServices":[o],"wantedServices":[]});
        Board::parse(&serde_json::to_vec(&board).unwrap(), Limits::default())
            .unwrap()
            .offerings
            .remove("page")
            .unwrap()
    }

    // The fixture's price and limits are read exactly and unknown compatible members are inert.
    #[test]
    fn the_acceptance_offering_parses() {
        let mut o = offering();
        let t = Terms::parse(&o).unwrap();
        assert_eq!(
            (t.price, t.maximum_path_bytes, t.maximum_resource_bytes),
            (4, 1024, 100)
        );
        o.source["info"]["future"] = true.into();
        assert_eq!(Terms::parse(&o), Ok(t));
    }

    // Every required positive member rejects zero, a missing value, a negative, a fraction and a string, and the path limit has an upper bound.
    #[test]
    fn every_member_is_checked() {
        for (parent, name) in [
            ("", "price"),
            ("info", "maximumPathBytes"),
            ("info", "maximumResourceBytes"),
        ] {
            for bad in [
                serde_json::json!(0),
                serde_json::json!(-1),
                serde_json::json!(1.5),
                serde_json::json!("1"),
                Value::Null,
            ] {
                let mut o = offering();
                if parent.is_empty() {
                    o.source[name] = bad.clone();
                } else {
                    o.source[parent][name] = bad.clone();
                }
                assert_eq!(Terms::parse(&o), Err(Error::Invalid), "{name} = {bad}");
            }
            let mut o = offering();
            if parent.is_empty() {
                o.source.as_object_mut().unwrap().remove(name);
            } else {
                o.source[parent].as_object_mut().unwrap().remove(name);
            }
            assert_eq!(Terms::parse(&o), Err(Error::Invalid), "{name} missing");
        }
        let mut o = offering();
        o.source["info"]["maximumPathBytes"] = 65_536.into();
        assert_eq!(Terms::parse(&o), Err(Error::Invalid));
        o.source["info"]["maximumPathBytes"] = 65_535.into();
        assert!(Terms::parse(&o).is_ok());
    }

    // A pricing object, a check price, a missing identity precondition, a wanted offering, another service or another version are refused.
    #[test]
    fn forbidden_shapes_are_refused() {
        let cases: [fn(&mut Offering); 5] = [
            |o| o.source["pricing"] = serde_json::json!({"model": "x"}),
            |o| o.source["checkPrice"] = 1.into(),
            |o| o.source["preconditions"] = serde_json::json!([]),
            |o| o.direction = crate::modules::board::offering::Direction::Wanted,
            |o| o.source["preconditions"] = serde_json::json!(["other"]),
        ];
        for edit in cases {
            let mut o = offering();
            edit(&mut o);
            assert_eq!(Terms::parse(&o), Err(Error::Invalid));
        }
        let mut o = offering();
        o.service = "cr2se.storage".into();
        assert_eq!(Terms::parse(&o), Err(Error::Unsupported));
        let mut o = offering();
        o.version = 2;
        assert_eq!(Terms::parse(&o), Err(Error::Unsupported));
    }
}
