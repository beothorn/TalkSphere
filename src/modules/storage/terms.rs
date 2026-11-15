//! Storage-specific current offering validation and immutable exact economic terms.
use super::pricing::{self, Rate};
use crate::modules::{
    board::offering::{Offering, integer},
    services::Error,
};
use serde_json::Value;

/// Validated Storage version-one economic and per-operation terms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Terms {
    /// Floor of calculated operation prices, in whole credits.
    pub minimum_price: u64,
    /// One-time uploaded byte charge.
    pub upload: Rate,
    /// Initial byte-second retention charge.
    pub retention: Rate,
    /// Returned byte charge.
    pub retrieval: Rate,
    /// Additional byte-second retention charge.
    pub renewal: Rate,
    /// Exact positive removal charge.
    pub remove_price: u64,
    /// Exact positive separately paid byte-check charge.
    pub check_price: u64,
    /// Maximum positive store size, in logical bytes.
    pub maximum_bytes: u64,
    /// Minimum positive initial/additional duration, in seconds.
    pub minimum_seconds: u64,
    /// Maximum positive initial/additional duration, in seconds.
    pub maximum_seconds: u64,
    /// Latest renewal relative to original acceptance, in seconds.
    pub maximum_total_seconds: u64,
    /// Maximum positive retrieval range, in bytes.
    pub maximum_retrieve_bytes: u64,
    /// Maximum requester-observed byte-check response time, in seconds.
    pub check_response_seconds: u64,
}
impl Terms {
    /// Validate Storage economics after common Board validation, including all advertised maxima.
    /// Unknown compatible fields remain inert; fixed pricing is forbidden.
    /// # Errors
    /// Unsupported identifies a different family/version/model; Invalid malformed terms; Limit price overflow.
    pub fn parse(offering: &Offering) -> Result<Self, Error> {
        tracing::trace!("Validating current Storage offering and maximum prices");
        let v = &offering.source;
        if offering.service != "cr2se.storage"
            || offering.version != 1
            || v["pricing"]["model"] != "cr2se.storage.v1"
        {
            return Err(Error::Unsupported);
        }
        if v.get("price").is_some()
            || !v["preconditions"]
                .as_array()
                .is_some_and(|a| a.iter().any(|x| x == "cr2se.identity"))
        {
            return Err(Error::Invalid);
        }
        let p = &v["pricing"];
        let i = &v["info"];
        let t = Self {
            minimum_price: positive(p, "minimumPrice")?,
            upload: rate(p, "byteUsageFactor")?,
            retention: rate(p, "byteStorageDurationFactor")?,
            retrieval: rate(p, "retrievalByteUsageFactor")?,
            renewal: rate(p, "renewalByteStorageDurationFactor")?,
            remove_price: positive(p, "removePrice")?,
            check_price: positive(v, "checkPrice")?,
            maximum_bytes: positive(i, "maximumBytes")?,
            minimum_seconds: positive(i, "minimumRetentionSeconds")?,
            maximum_seconds: positive(i, "maximumRetentionSeconds")?,
            maximum_total_seconds: positive(i, "maximumTotalRetentionSeconds")?,
            maximum_retrieve_bytes: positive(i, "maximumRetrieveBytes")?,
            check_response_seconds: positive(i, "checkResponseSeconds")?,
        };
        if t.minimum_seconds > t.maximum_seconds || t.maximum_seconds > t.maximum_total_seconds {
            return Err(Error::Invalid);
        }
        pricing::store(
            t.maximum_bytes,
            t.maximum_seconds,
            t.upload,
            t.retention,
            t.minimum_price,
        )?;
        pricing::usage(t.maximum_retrieve_bytes, 1, t.retrieval, t.minimum_price)?;
        pricing::usage(
            t.maximum_bytes,
            t.maximum_seconds,
            t.renewal,
            t.minimum_price,
        )?;
        Ok(t)
    }
}
// Decode exact positive uint64 terms without decimal or floating-point coercion.
fn positive(v: &Value, name: &str) -> Result<u64, Error> {
    tracing::trace!("Reading positive Storage term");
    v.get(name)
        .and_then(integer)
        .filter(|n| *n > 0)
        .ok_or(Error::Invalid)
}
// Required rational fields retain zero numerators and reject absent/invalid denominators.
fn rate(v: &Value, name: &str) -> Result<Rate, Error> {
    tracing::trace!("Reading exact Storage rate");
    let r = v.get(name).ok_or(Error::Invalid)?;
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
    // Build a syntax-valid fixture independently from family validation.
    fn offering() -> Offering {
        let o: Value = serde_json::from_str(include_str!("fixtures/offering.json")).unwrap();
        let b = serde_json::json!({"version":1,"providedServices":[o],"wantedServices":[]});
        Board::parse(&serde_json::to_vec(&b).unwrap(), Limits::default())
            .unwrap()
            .offerings
            .remove("storage-standard")
            .unwrap()
    }
    // Both roles and compatible additions preserve the same exact economic terms.
    #[test]
    fn current_terms_and_compatible_extensions() {
        let mut o = offering();
        let t = Terms::parse(&o).unwrap();
        assert_eq!(t.maximum_bytes, 100);
        assert_eq!(t.check_price, 3);
        o.direction = Direction::Wanted;
        o.source["info"]["future"] = true.into();
        assert_eq!(Terms::parse(&o), Ok(t));
        for f in [
            "byteUsageFactor",
            "byteStorageDurationFactor",
            "retrievalByteUsageFactor",
            "renewalByteStorageDurationFactor",
        ] {
            o.source["pricing"][f]["numerator"] = 0.into();
        }
        assert_eq!(Terms::parse(&o).unwrap().upload.numerator, 0);
    }
    // Every required positive field and all advertised arithmetic maxima are checked before use.
    #[test]
    fn invalid_and_overflowing_terms() {
        for (parent, names) in [
            ("pricing", vec!["minimumPrice", "removePrice"]),
            (
                "info",
                vec![
                    "maximumBytes",
                    "minimumRetentionSeconds",
                    "maximumRetentionSeconds",
                    "maximumTotalRetentionSeconds",
                    "maximumRetrieveBytes",
                    "checkResponseSeconds",
                ],
            ),
        ] {
            for name in names {
                let mut o = offering();
                o.source[parent][name] = 0.into();
                assert_eq!(Terms::parse(&o), Err(Error::Invalid));
            }
        }
        let mut o = offering();
        o.source["checkPrice"] = 0.into();
        assert_eq!(Terms::parse(&o), Err(Error::Invalid));
        let mut o = offering();
        o.source["pricing"]["byteUsageFactor"]["denominator"] = 0.into();
        assert_eq!(Terms::parse(&o), Err(Error::Invalid));
        let mut o = offering();
        o.source["pricing"]["byteUsageFactor"]["numerator"] = (-1).into();
        assert_eq!(Terms::parse(&o), Err(Error::Invalid));
        let mut o = offering();
        o.source["info"]["maximumBytes"] = u64::MAX.into();
        assert_eq!(Terms::parse(&o), Err(Error::Limit));
        let mut o = offering();
        o.source["info"]["minimumRetentionSeconds"] = 101.into();
        assert_eq!(Terms::parse(&o), Err(Error::Invalid));
        let mut o = offering();
        o.source["info"]["maximumTotalRetentionSeconds"] = 99.into();
        assert_eq!(Terms::parse(&o), Err(Error::Invalid));
        let mut o = offering();
        o.source["price"] = 1.into();
        assert_eq!(Terms::parse(&o), Err(Error::Invalid));
        let mut o = offering();
        o.source["preconditions"] = serde_json::json!([]);
        assert_eq!(Terms::parse(&o), Err(Error::Invalid));
        let mut o = offering();
        o.version = 2;
        assert_eq!(Terms::parse(&o), Err(Error::Unsupported));
        let mut o = offering();
        o.service = "other".into();
        assert_eq!(Terms::parse(&o), Err(Error::Unsupported));
        let mut o = offering();
        o.source["pricing"]["model"] = "other".into();
        assert_eq!(Terms::parse(&o), Err(Error::Unsupported));
    }
}
