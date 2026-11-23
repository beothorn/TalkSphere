//! Validation of a Hosted Publications offering into immutable exact economic terms and limits, and the exact prices they imply.
//!
//! Callers: provider registration, quoting and the requester. Validation happens after the common Board checks and rejects any
//! value the module could not honor, including advertised maxima whose prices would overflow uint64, before the offering is
//! published or used. The price arithmetic is Messaging's exact rational ceiling with a minimum, reused through its public helpers.
use super::{error::Error, signing::MAXIMUM_CONTENT_BYTES};
use crate::modules::{
    board::offering::{Offering, integer},
    identity::api::IdentityId,
    messaging::pricing::{self, Rate},
};
use serde_json::Value;

/// Service identifier of version one.
pub const SERVICE: &str = "cr2se.hosted-publication";
/// The only supported service version.
pub const VERSION: u32 = 1;
/// The only supported pricing model.
pub const PRICING_MODEL: &str = "cr2se.hosted-publication.v1";
/// Largest audience an offering may advertise (the specification's `0 .. 65535`).
pub const MAXIMUM_AUDIENCE_LIMIT: u64 = 65_535;

/// Validated Hosted Publications version-one terms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Terms {
    /// Floor of calculated store, get and renew prices.
    pub minimum_price: u64,
    /// One-time content byte charge of a store.
    pub content_usage: Rate,
    /// Initial content byte-second retention charge of a store.
    pub content_storage: Rate,
    /// Byte charge of a get.
    pub read_usage: Rate,
    /// Renewal byte-second charge.
    pub renewal_storage: Rate,
    /// Exact price of one successful list.
    pub list_price: u64,
    /// Exact price of one successful remove.
    pub remove_price: u64,
    /// Exact price of one successful check.
    pub check_price: u64,
    /// Largest accepted content.
    pub maximum_content_bytes: u64,
    /// Smallest initial or renewal duration.
    pub minimum_retention_seconds: u64,
    /// Largest duration of one store or renewal.
    pub maximum_retention_seconds: u64,
    /// Latest expiry relative to the original acceptance.
    pub maximum_total_retention_seconds: u64,
    /// Largest list page.
    pub maximum_list_results: u32,
    /// Most readers of a restricted publication; zero means public only.
    pub maximum_audience: u16,
    /// Most resources a page manifest may name.
    pub maximum_manifest_resources: u32,
    /// Longest requester-observed time for check evidence.
    pub check_response_seconds: u64,
}

impl Terms {
    /// Validate a provided offering published by `publisher`.
    /// # Errors
    /// `Unsupported` for another service, version or pricing model; `Invalid` for a missing, zero, inconsistent or out-of-range
    /// field, a fixed `price`, a missing `cr2se.identity` precondition, an issuer other than the publisher or a content maximum
    /// above the finite profile's 16384 bytes; `Limit` when an advertised maximum would make a price exceed uint64.
    pub fn parse(offering: &Offering, publisher: IdentityId) -> Result<Self, Error> {
        tracing::trace!("Validating a Hosted Publications offering and its maximum prices");
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
        let terms = Self {
            minimum_price: positive(p, "minimumPrice")?,
            content_usage: rate(p, "contentByteUsageFactor")?,
            content_storage: rate(p, "contentByteStorageDurationFactor")?,
            read_usage: rate(p, "readByteUsageFactor")?,
            renewal_storage: rate(p, "renewalByteStorageDurationFactor")?,
            list_price: positive(p, "listPrice")?,
            remove_price: positive(p, "removePrice")?,
            check_price: positive(v, "checkPrice")?,
            maximum_content_bytes: positive(i, "maximumContentBytes")?,
            minimum_retention_seconds: positive(i, "minimumRetentionSeconds")?,
            maximum_retention_seconds: positive(i, "maximumRetentionSeconds")?,
            maximum_total_retention_seconds: positive(i, "maximumTotalRetentionSeconds")?,
            maximum_list_results: u32::try_from(positive(i, "maximumListResults")?)
                .map_err(|_| Error::Invalid)?,
            maximum_audience: audience(i)?,
            maximum_manifest_resources: u32::try_from(positive(i, "maximumManifestResources")?)
                .map_err(|_| Error::Invalid)?,
            check_response_seconds: positive(i, "checkResponseSeconds")?,
        };
        if terms.maximum_content_bytes > MAXIMUM_CONTENT_BYTES as u64
            || terms.minimum_retention_seconds > terms.maximum_retention_seconds
            || terms.maximum_retention_seconds > terms.maximum_total_retention_seconds
        {
            return Err(Error::Invalid);
        }
        // Every price must be representable at the advertised maxima.
        terms.store_price(terms.maximum_content_bytes, terms.maximum_retention_seconds)?;
        terms.get_price(terms.maximum_content_bytes)?;
        terms.renew_price(terms.maximum_content_bytes, terms.maximum_retention_seconds)?;
        Ok(terms)
    }

    /// Price of storing `size` content bytes for `seconds`: `max(minimum, ceil(size*B + size*seconds*X))`.
    /// # Errors
    /// `Invalid` for zero size or seconds; `Limit` when the exact result exceeds uint64.
    pub fn store_price(&self, size: u64, seconds: u64) -> Result<u64, Error> {
        tracing::trace!("Calculating the exact hosted store price");
        Ok(pricing::store(
            size,
            seconds,
            self.content_usage,
            self.content_storage,
            self.minimum_price,
        )?)
    }

    /// Price of returning `size` content bytes: `max(minimum, ceil(size*R))`.
    /// # Errors
    /// `Invalid` for zero size; `Limit` when the exact result exceeds uint64.
    pub fn get_price(&self, size: u64) -> Result<u64, Error> {
        tracing::trace!("Calculating the exact hosted get price");
        Ok(pricing::recover(size, self.read_usage, self.minimum_price)?)
    }

    /// Price of adding `seconds` of retention to `size` content bytes: `max(minimum, ceil(size*seconds*N))`.
    /// # Errors
    /// `Invalid` for zero size or seconds; `Limit` when the exact result exceeds uint64.
    pub fn renew_price(&self, size: u64, seconds: u64) -> Result<u64, Error> {
        tracing::trace!("Calculating the exact hosted renewal price");
        Ok(pricing::renew(
            size,
            seconds,
            self.renewal_storage,
            self.minimum_price,
        )?)
    }
}

// An exact positive uint64 member, rejecting decimals, negatives, strings and zero.
fn positive(value: &Value, name: &str) -> Result<u64, Error> {
    tracing::trace!("Reading a positive hosted publication term");
    value
        .get(name)
        .and_then(integer)
        .filter(|n| *n > 0)
        .ok_or(Error::Invalid)
}

// The audience limit: an exact integer in `0 ..= 65535`, where zero means public publications only.
fn audience(info: &Value) -> Result<u16, Error> {
    tracing::trace!("Reading the hosted audience limit");
    info.get("maximumAudience")
        .and_then(integer)
        .filter(|n| *n <= MAXIMUM_AUDIENCE_LIMIT)
        .and_then(|n| u16::try_from(n).ok())
        .ok_or(Error::Invalid)
}

// A required rational: any exact numerator including zero, and a positive denominator.
fn rate(value: &Value, name: &str) -> Result<Rate, Error> {
    tracing::trace!("Reading an exact hosted rate");
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
            .remove("hosted-publication-standard")
            .unwrap()
    }

    /// The validated terms of the acceptance offering, for provider tests.
    pub(crate) fn terms() -> Terms {
        Terms::parse(&offering(), PROVIDER).unwrap()
    }

    // The fixture's economics and limits are read exactly, and compatible additions and the wanted role keep the same terms.
    #[test]
    fn the_acceptance_offering_parses_and_extensions_are_inert() {
        let mut o = offering();
        let t = Terms::parse(&o, PROVIDER).unwrap();
        assert_eq!(
            (t.minimum_price, t.list_price, t.remove_price, t.check_price),
            (1, 1, 2, 3)
        );
        assert_eq!(
            (
                t.maximum_content_bytes,
                t.maximum_list_results,
                t.maximum_audience,
                t.maximum_manifest_resources,
                t.check_response_seconds
            ),
            (100, 2, 4, 8, 5)
        );
        o.direction = Direction::Wanted;
        o.source["info"]["future"] = true.into();
        assert_eq!(Terms::parse(&o, PROVIDER), Ok(t));
        // A zero audience limit means public publications only; a zero numerator is a free usage term.
        o.source["info"]["maximumAudience"] = 0.into();
        o.source["pricing"]["readByteUsageFactor"]["numerator"] = 0.into();
        let parsed = Terms::parse(&o, PROVIDER).unwrap();
        assert_eq!(
            (parsed.maximum_audience, parsed.read_usage.numerator),
            (0, 0)
        );
    }

    // HP-06 and HP-02 prices: store of `abc` for 10 s is 17, get of 3 bytes is 2, renewal of 3 bytes by 10 s is 15, all floored at 1.
    #[test]
    fn prices_follow_the_rational_ceiling_formulas() {
        let t = terms();
        assert_eq!(t.store_price(3, 10), Ok(17));
        assert_eq!(t.get_price(3), Ok(2));
        assert_eq!(t.get_price(1), Ok(1));
        assert_eq!(t.renew_price(3, 10), Ok(15));
        assert_eq!(t.store_price(0, 10), Err(Error::Invalid));
        assert_eq!(t.store_price(1, 0), Err(Error::Invalid));
        assert_eq!(t.get_price(0), Err(Error::Invalid));
        assert_eq!(t.renew_price(1, 0), Err(Error::Invalid));
        let huge = Terms {
            content_usage: Rate {
                numerator: u64::MAX,
                denominator: 1,
            },
            ..t
        };
        assert_eq!(huge.store_price(u64::MAX, u64::MAX), Err(Error::Limit));
    }

    // Every required positive member rejects zero, a missing value, a negative, a fraction and a string.
    #[test]
    fn every_required_member_is_checked() {
        let members = [
            ("pricing", "minimumPrice"),
            ("pricing", "listPrice"),
            ("pricing", "removePrice"),
            ("info", "maximumContentBytes"),
            ("info", "minimumRetentionSeconds"),
            ("info", "maximumRetentionSeconds"),
            ("info", "maximumTotalRetentionSeconds"),
            ("info", "maximumListResults"),
            ("info", "maximumManifestResources"),
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
        for (name, value) in [
            ("maximumListResults", 4_294_967_296u64),
            ("maximumManifestResources", 4_294_967_296),
            ("maximumAudience", 65_536),
        ] {
            let mut o = offering();
            o.source["info"][name] = value.into();
            assert_eq!(Terms::parse(&o, PROVIDER), Err(Error::Invalid), "{name}");
        }
        let mut o = offering();
        o.source["info"]
            .as_object_mut()
            .unwrap()
            .remove("maximumAudience");
        assert_eq!(Terms::parse(&o, PROVIDER), Err(Error::Invalid));
    }

    // Rates need a positive denominator and an exact nonnegative numerator, for each of the four factors.
    #[test]
    fn rates_are_validated() {
        for name in [
            "contentByteUsageFactor",
            "contentByteStorageDurationFactor",
            "readByteUsageFactor",
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

    // Duration ordering, the content bound, the fixed-price field, the identity precondition and the issuer are all enforced.
    #[test]
    fn consistency_rules_are_enforced() {
        let cases: [fn(&mut Offering); 7] = [
            |o| o.source["info"]["minimumRetentionSeconds"] = 101.into(),
            |o| o.source["info"]["maximumTotalRetentionSeconds"] = 99.into(),
            |o| o.source["info"]["maximumContentBytes"] = 16_385.into(),
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
        assert_eq!(
            Terms::parse(&offering(), IdentityId([3; 32])),
            Err(Error::Invalid)
        );
        // The profile bound itself is allowed.
        let mut o = offering();
        o.source["info"]["maximumContentBytes"] = 16_384.into();
        assert!(Terms::parse(&o, PROVIDER).is_ok());
    }

    // Another service, version or pricing model is unsupported, and maxima that overflow any price are a limit failure.
    #[test]
    fn unsupported_models_and_overflowing_maxima() {
        let mut o = offering();
        o.service = "cr2se.messaging".into();
        assert_eq!(Terms::parse(&o, PROVIDER), Err(Error::Unsupported));
        let mut o = offering();
        o.version = 2;
        assert_eq!(Terms::parse(&o, PROVIDER), Err(Error::Unsupported));
        let mut o = offering();
        o.source["pricing"]["model"] = "other".into();
        assert_eq!(Terms::parse(&o, PROVIDER), Err(Error::Unsupported));
        let mut o = offering();
        o.source["pricing"]["contentByteStorageDurationFactor"]["numerator"] = u64::MAX.into();
        o.source["info"]["maximumRetentionSeconds"] = u64::MAX.into();
        o.source["info"]["maximumTotalRetentionSeconds"] = u64::MAX.into();
        assert_eq!(Terms::parse(&o, PROVIDER), Err(Error::Limit));
    }
}
