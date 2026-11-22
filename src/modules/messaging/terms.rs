//! Validation of a Messaging offering into immutable exact economic terms and limits.
//!
//! Callers: provider registration, quoting and the requester. Validation happens after the common Board
//! checks and rejects any value the module could not honor, including advertised maxima whose prices
//! would overflow uint64, before the offering is published or used.
use super::{
    error::Error,
    pricing::{self, Rate},
    state::{PRICING_MODEL, SERVICE, VERSION},
};
use crate::modules::{
    board::offering::{Offering, integer},
    identity::api::IdentityId,
};
use serde_json::Value;

/// Validated Messaging version-one terms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Terms {
    /// Floor of calculated store, recover and renew prices.
    pub minimum_price: u64,
    /// One-time payload byte charge.
    pub payload_usage: Rate,
    /// Initial payload byte-second retention charge.
    pub payload_storage: Rate,
    /// Recovery byte charge.
    pub recovery_usage: Rate,
    /// Renewal byte-second charge.
    pub renewal_storage: Rate,
    /// Exact price of one successful discover.
    pub discover_price: u64,
    /// Exact price of one successful status.
    pub status_price: u64,
    /// Exact price of one successful remove.
    pub remove_price: u64,
    /// Exact price of one successful check.
    pub check_price: u64,
    /// Largest accepted payload.
    pub maximum_payload_bytes: u64,
    /// Smallest initial or renewal duration.
    pub minimum_retention_seconds: u64,
    /// Largest duration of one store or renewal.
    pub maximum_retention_seconds: u64,
    /// Latest expiry relative to the original acceptance.
    pub maximum_total_retention_seconds: u64,
    /// Largest discovery page.
    pub maximum_discover_results: u32,
    /// Longest requester-observed time for check evidence.
    pub check_response_seconds: u64,
}

impl Terms {
    /// Validate a provided offering published by `publisher`.
    /// # Errors
    /// `Unsupported` for another service, version or pricing model; `Invalid` for a missing, zero, duplicate-meaning or
    /// inconsistent field, a fixed `price`, a missing `cr2se.identity` precondition or an issuer other than the
    /// publisher; `Limit` when an advertised maximum would make a price exceed uint64.
    pub fn parse(offering: &Offering, publisher: IdentityId) -> Result<Self, Error> {
        tracing::trace!("Validating a Messaging offering and its maximum prices");
        let v = &offering.source;
        if offering.service != SERVICE
            || offering.version != VERSION
            || v["pricing"]["model"] != PRICING_MODEL
        {
            return Err(Error::Unsupported);
        }
        if v.get("price").is_some()
            || offering.issuer != publisher
            || !v["preconditions"]
                .as_array()
                .is_some_and(|a| a.iter().any(|x| x == "cr2se.identity"))
        {
            return Err(Error::Invalid);
        }
        let (p, i) = (&v["pricing"], &v["info"]);
        let discover = positive(i, "maximumDiscoverResults")?;
        let terms = Self {
            minimum_price: positive(p, "minimumPrice")?,
            payload_usage: rate(p, "payloadByteUsageFactor")?,
            payload_storage: rate(p, "payloadByteStorageDurationFactor")?,
            recovery_usage: rate(p, "recoveryByteUsageFactor")?,
            renewal_storage: rate(p, "renewalByteStorageDurationFactor")?,
            discover_price: positive(p, "discoverPrice")?,
            status_price: positive(p, "statusPrice")?,
            remove_price: positive(p, "removePrice")?,
            check_price: positive(v, "checkPrice")?,
            maximum_payload_bytes: positive(i, "maximumPayloadBytes")?,
            minimum_retention_seconds: positive(i, "minimumRetentionSeconds")?,
            maximum_retention_seconds: positive(i, "maximumRetentionSeconds")?,
            maximum_total_retention_seconds: positive(i, "maximumTotalRetentionSeconds")?,
            maximum_discover_results: u32::try_from(discover).map_err(|_| Error::Invalid)?,
            check_response_seconds: positive(i, "checkResponseSeconds")?,
        };
        if terms.minimum_retention_seconds > terms.maximum_retention_seconds
            || terms.maximum_retention_seconds > terms.maximum_total_retention_seconds
        {
            return Err(Error::Invalid);
        }
        // Every price must be representable at the advertised maxima.
        pricing::store(
            terms.maximum_payload_bytes,
            terms.maximum_retention_seconds,
            terms.payload_usage,
            terms.payload_storage,
            terms.minimum_price,
        )?;
        pricing::recover(
            terms.maximum_payload_bytes,
            terms.recovery_usage,
            terms.minimum_price,
        )?;
        pricing::renew(
            terms.maximum_payload_bytes,
            terms.maximum_retention_seconds,
            terms.renewal_storage,
            terms.minimum_price,
        )?;
        Ok(terms)
    }
}

// An exact positive uint64 member, rejecting decimals, negatives, strings and zero.
fn positive(value: &Value, name: &str) -> Result<u64, Error> {
    tracing::trace!("Reading a positive Messaging term");
    value
        .get(name)
        .and_then(integer)
        .filter(|n| *n > 0)
        .ok_or(Error::Invalid)
}

// A required rational: any exact numerator including zero, and a positive denominator.
fn rate(value: &Value, name: &str) -> Result<Rate, Error> {
    tracing::trace!("Reading an exact Messaging rate");
    let r = value.get(name).ok_or(Error::Invalid)?;
    Ok(Rate {
        numerator: r.get("numerator").and_then(integer).ok_or(Error::Invalid)?,
        denominator: positive(r, "denominator")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::board::{
        config::Limits,
        offering::{Board, Direction},
    };

    const PROVIDER: IdentityId = IdentityId([2; 32]);

    // The acceptance-harness offering issued by the provider test identity.
    fn offering() -> Offering {
        let mut o: Value = serde_json::from_str(include_str!("fixtures/offering.json")).unwrap();
        o["creditIssuer"] = crate::modules::identity::text::format(PROVIDER).into();
        let board = serde_json::json!({"version":1,"providedServices":[o],"wantedServices":[]});
        Board::parse(&serde_json::to_vec(&board).unwrap(), Limits::default())
            .unwrap()
            .offerings
            .remove("messaging-standard")
            .unwrap()
    }

    // The fixture's economics and limits are read exactly, and compatible additions and the wanted role keep the same terms.
    #[test]
    fn the_acceptance_offering_parses_and_extensions_are_inert() {
        let mut o = offering();
        let t = Terms::parse(&o, PROVIDER).unwrap();
        assert_eq!(
            (
                t.minimum_price,
                t.discover_price,
                t.status_price,
                t.remove_price,
                t.check_price
            ),
            (1, 1, 1, 2, 3)
        );
        assert_eq!(
            (
                t.maximum_payload_bytes,
                t.maximum_discover_results,
                t.check_response_seconds
            ),
            (100, 100, 5)
        );
        assert_eq!(
            t.payload_usage,
            Rate {
                numerator: 1,
                denominator: 2
            }
        );
        o.direction = Direction::Wanted;
        o.source["info"]["future"] = true.into();
        assert_eq!(Terms::parse(&o, PROVIDER), Ok(t));
        o.source["pricing"]["payloadByteUsageFactor"]["numerator"] = 0.into();
        assert_eq!(
            Terms::parse(&o, PROVIDER).unwrap().payload_usage.numerator,
            0
        );
    }

    // Every required positive member rejects zero, a missing value, a negative, a fraction and a string.
    #[test]
    fn every_required_member_is_checked() {
        let members = [
            ("pricing", "minimumPrice"),
            ("pricing", "discoverPrice"),
            ("pricing", "statusPrice"),
            ("pricing", "removePrice"),
            ("info", "maximumPayloadBytes"),
            ("info", "minimumRetentionSeconds"),
            ("info", "maximumRetentionSeconds"),
            ("info", "maximumTotalRetentionSeconds"),
            ("info", "maximumDiscoverResults"),
            ("info", "checkResponseSeconds"),
        ];
        for (parent, name) in members {
            for bad in [
                serde_json::json!(0),
                serde_json::json!(-1),
                serde_json::json!(1.5),
                serde_json::json!("1"),
                Value::Null,
            ] {
                let mut o = offering();
                o.source[parent][name] = bad.clone();
                assert_eq!(
                    Terms::parse(&o, PROVIDER),
                    Err(Error::Invalid),
                    "{parent}.{name} = {bad}"
                );
            }
            let mut o = offering();
            o.source[parent].as_object_mut().unwrap().remove(name);
            assert_eq!(
                Terms::parse(&o, PROVIDER),
                Err(Error::Invalid),
                "{name} missing"
            );
        }
        let mut o = offering();
        o.source["checkPrice"] = 0.into();
        assert_eq!(Terms::parse(&o, PROVIDER), Err(Error::Invalid));
        let mut o = offering();
        o.source["info"]["maximumDiscoverResults"] = 4_294_967_296u64.into();
        assert_eq!(Terms::parse(&o, PROVIDER), Err(Error::Invalid));
    }

    // Rates need a positive denominator and an exact nonnegative numerator, for each of the four factors.
    #[test]
    fn rates_are_validated() {
        for name in [
            "payloadByteUsageFactor",
            "payloadByteStorageDurationFactor",
            "recoveryByteUsageFactor",
            "renewalByteStorageDurationFactor",
        ] {
            for (field, bad) in [("denominator", 0), ("numerator", -1)] {
                let mut o = offering();
                o.source["pricing"][name][field] = bad.into();
                assert_eq!(
                    Terms::parse(&o, PROVIDER),
                    Err(Error::Invalid),
                    "{name}.{field}"
                );
            }
            let mut o = offering();
            o.source["pricing"].as_object_mut().unwrap().remove(name);
            assert_eq!(Terms::parse(&o, PROVIDER), Err(Error::Invalid));
        }
    }

    // Duration ordering, the fixed-price field, the identity precondition and the issuer are all enforced.
    #[test]
    fn consistency_rules_are_enforced() {
        let cases: [fn(&mut Offering); 6] = [
            |o| o.source["info"]["minimumRetentionSeconds"] = 101.into(),
            |o| o.source["info"]["maximumTotalRetentionSeconds"] = 99.into(),
            |o| o.source["price"] = 1.into(),
            |o| o.source["preconditions"] = serde_json::json!([]),
            |o| o.source["preconditions"] = serde_json::json!(["other"]),
            |o| o.issuer = IdentityId([9; 32]),
        ];
        for edit in cases {
            let mut o = offering();
            edit(&mut o);
            assert_eq!(Terms::parse(&o, PROVIDER), Err(Error::Invalid));
        }
        // The issuer must be the publisher that is asked about.
        assert_eq!(
            Terms::parse(&offering(), IdentityId([3; 32])),
            Err(Error::Invalid)
        );
    }

    // Another service, version or pricing model is unsupported, and maxima that overflow any price are a limit failure.
    #[test]
    fn unsupported_models_and_overflowing_maxima() {
        let mut o = offering();
        o.service = "cr2se.storage".into();
        assert_eq!(Terms::parse(&o, PROVIDER), Err(Error::Unsupported));
        let mut o = offering();
        o.version = 2;
        assert_eq!(Terms::parse(&o, PROVIDER), Err(Error::Unsupported));
        let mut o = offering();
        o.source["pricing"]["model"] = "other".into();
        assert_eq!(Terms::parse(&o, PROVIDER), Err(Error::Unsupported));
        for (parent, name, value) in [
            ("info", "maximumPayloadBytes", u64::MAX),
            ("info", "maximumRetentionSeconds", u64::MAX),
        ] {
            let mut o = offering();
            o.source[parent][name] = value.into();
            o.source["info"]["maximumTotalRetentionSeconds"] = u64::MAX.into();
            assert_eq!(Terms::parse(&o, PROVIDER), Err(Error::Limit), "{name}");
        }
    }
}
