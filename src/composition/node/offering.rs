//! Builds the node's Storage offering and its independently recognized definition.
//!
//! Callers: owner opening. The terms come only from operator configuration; the Board
//! parser and the Storage standards catalogue validate them, so unrepresentable prices or
//! maxima are refused before anything is published.
use super::config::Terms;
use crate::composition::policy::Wanted;
use crate::modules::{
    board::{definition::Definition, offering::Board},
    identity::{api::IdentityId, text},
    storage::standard::Catalogue,
};
use serde_json::{Value, json};

/// Identifier of the node's one provided Storage offering.
pub const OFFERING_ID: &str = "storage-standard";
/// Identifier of the node's wanted Storage offering, published only when policy asks for contributions.
pub const WANTED_ID: &str = "storage-wanted";

/// A validated offering: the Board document bytes and its complete standard definition.
pub struct Prepared {
    /// Canonical Board document with one provided Storage offering, or none.
    pub board: Vec<u8>,
    /// The standard definition both requester and provider install.
    pub definition: Definition,
    /// Bytes of the definition document, published beside the Board.
    pub definition_bytes: Vec<u8>,
    /// Bytes of the wanted offering's definition when one is published.
    pub wanted_definition_bytes: Option<Vec<u8>>,
}

/// Build and validate the offering for `local`; `terms` of `None` yields an empty Board
/// plus the default definition, which a requester-only node still needs for its client.
/// # Errors
/// Terms the Board parser or the Storage catalogue cannot represent are refused with a
/// message naming the failing stage, never the values.
pub fn prepare(
    local: IdentityId,
    terms: Option<&Terms>,
    wanted: Option<&Wanted>,
    catalogue: &Catalogue,
) -> Result<Prepared, String> {
    tracing::trace!("Preparing the node Storage offerings");
    let base = terms.map_or_else(
        || {
            super::config::NodeConfig::default()
                .offering
                .ok_or("no default offering")
        },
        |t| Ok(t.clone()),
    )?;
    let provided = offering(local, &base);
    let wanted_offering = wanted.map(|w| wanted_offering(local, &base, w));
    // The definition always comes from a fully validated offering, even when none is published.
    let all = serde_json::to_vec(&json!({
        "version": 1, "providedServices": [provided],
        "wantedServices": wanted_offering.iter().collect::<Vec<_>>(),
    }))
    .map_err(|_| "offering could not be encoded")?;
    let parsed =
        Board::parse(&all, Default::default()).map_err(|_| "offering terms are invalid")?;
    let offering = parsed
        .offerings
        .get(OFFERING_ID)
        .ok_or("offering is missing after parsing")?;
    // The Storage owner's own term checks reject advertised maxima and prices the family cannot honor.
    crate::modules::storage::terms::Terms::parse(offering)
        .map_err(|_| "offering terms are not valid Storage terms")?;
    let definition = catalogue
        .definition(offering)
        .map_err(|_| "offering terms are not a valid Storage standard definition")?;
    let definition_bytes = definition.document.bytes().to_vec();
    let wanted_definition_bytes = match &wanted_offering {
        None => None,
        Some(_) => {
            let wanted = parsed
                .offerings
                .get(WANTED_ID)
                .ok_or("wanted offering is missing after parsing")?;
            crate::modules::storage::terms::Terms::parse(wanted)
                .map_err(|_| "wanted terms are not valid Storage terms")?;
            Some(
                catalogue
                    .definition(wanted)
                    .map_err(|_| "wanted terms are not a valid Storage standard definition")?
                    .document
                    .bytes()
                    .to_vec(),
            )
        }
    };
    let board = serde_json::to_vec(&json!({
        "version": 1,
        "providedServices": if terms.is_some() { vec![provided] } else { vec![] },
        "wantedServices": wanted_offering.iter().collect::<Vec<_>>(),
    }))
    .map_err(|_| "offering could not be encoded")?;
    Ok(Prepared {
        board,
        definition,
        definition_bytes,
        wanted_definition_bytes,
    })
}

// The wanted offering states what the publisher pays as requester: a fixed number of its own credits per
// object. Factors are the smallest positive fractions, so the exact price is the minimum price whenever
// the object and retention are within the advertised bounds.
fn wanted_offering(local: IdentityId, base: &Terms, wanted: &Wanted) -> Value {
    tracing::trace!("Assembling the wanted Storage offering object");
    let tiny = json!({"numerator": 1, "denominator": u64::MAX});
    let bytes = wanted
        .maximum_object_bytes
        .min(base.info["maximumBytes"].as_u64().unwrap_or(u64::MAX));
    let retention = wanted.retention_seconds;
    json!({
        "id": WANTED_ID,
        "service": "cr2se.storage",
        "serviceVersion": 1,
        // The description is part of the contract identity the installed Storage family recognizes, so it
        // must match the provided offering exactly.
        "description": "Store requester-selected data for a requester-selected period.",
        "creditIssuer": text::format(local),
        "pricing": {
            "model": "cr2se.storage.v1",
            "minimumPrice": wanted.credits_per_object,
            "byteUsageFactor": tiny,
            "byteStorageDurationFactor": tiny,
            "retrievalByteUsageFactor": tiny,
            "renewalByteStorageDurationFactor": tiny,
            "removePrice": 1,
        },
        "checkPrice": 1,
        "info": {
            "maximumBytes": bytes,
            "minimumRetentionSeconds": 1,
            "maximumRetentionSeconds": retention,
            "maximumTotalRetentionSeconds": retention.saturating_mul(2).max(retention),
            "maximumRetrieveBytes": bytes,
            "checkResponseSeconds": 5,
        },
        "preconditions": ["cr2se.identity"],
    })
}

// Complete offering object with the local identity as the credit issuer.
fn offering(local: IdentityId, terms: &Terms) -> Value {
    tracing::trace!("Assembling the Storage offering object");
    json!({
        "id": OFFERING_ID,
        "service": "cr2se.storage",
        "serviceVersion": 1,
        "description": "Store requester-selected data for a requester-selected period.",
        "creditIssuer": text::format(local),
        "pricing": terms.pricing,
        "checkPrice": terms.check_price,
        "info": terms.info,
        "preconditions": ["cr2se.identity"],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composition::node::config::NodeConfig;
    fn catalogue() -> Catalogue {
        Catalogue::new(Default::default()).unwrap()
    }
    // Default terms yield a one-offering Board and a definition; a requester-only node publishes none.
    #[test]
    fn default_terms_publish_one_offering_and_requester_only_publishes_none() {
        let config = NodeConfig::default();
        let me = IdentityId([1; 32]);
        let provided = prepare(me, config.offering.as_ref(), None, &catalogue()).unwrap();
        let text = String::from_utf8(provided.board).unwrap();
        assert!(text.contains("storage-standard") && text.contains(&super::text::format(me)));
        assert!(!provided.definition_bytes.is_empty());
        let none = prepare(me, None, None, &catalogue()).unwrap();
        assert!(String::from_utf8(none.board).unwrap().contains("[]"));
        assert!(!none.definition_bytes.is_empty());
    }
    // A wanted offering is published beside the provided one with its own definition, issued in the
    // publisher's credits, and a requester-only node can publish a wanted offering alone.
    #[test]
    fn wanted_terms_publish_a_second_offering() {
        let me = IdentityId([1; 32]);
        let wanted = Wanted {
            credits_per_object: 7,
            maximum_object_bytes: 4096,
            retention_seconds: 3600,
        };
        let config = NodeConfig::default();
        let both = prepare(me, config.offering.as_ref(), Some(&wanted), &catalogue()).unwrap();
        let text = String::from_utf8(both.board).unwrap();
        assert!(text.contains("storage-standard") && text.contains(WANTED_ID));
        assert!(both.wanted_definition_bytes.is_some_and(|b| !b.is_empty()));
        let only = prepare(me, None, Some(&wanted), &catalogue()).unwrap();
        let board: Value = serde_json::from_slice(&only.board).unwrap();
        assert!(board["providedServices"].as_array().unwrap().is_empty());
        assert_eq!(board["wantedServices"][0]["pricing"]["minimumPrice"], 7);
        assert_eq!(board["wantedServices"][0]["info"]["maximumBytes"], 4096);
        let none = prepare(me, None, None, &catalogue()).unwrap();
        assert!(none.wanted_definition_bytes.is_none());
    }
    // Wanted terms the Storage family cannot honor are refused before anything is published.
    #[test]
    fn unrepresentable_wanted_terms_are_refused() {
        let me = IdentityId([1; 32]);
        let config = NodeConfig::default();
        let wanted = Wanted {
            credits_per_object: 7,
            maximum_object_bytes: 4096,
            retention_seconds: 0,
        };
        assert!(prepare(me, config.offering.as_ref(), Some(&wanted), &catalogue()).is_err());
    }
    // Unrepresentable prices and maxima are refused before publication, without echoing values.
    #[test]
    fn invalid_terms_are_refused() {
        let me = IdentityId([1; 32]);
        let mut bad = NodeConfig::default().offering.unwrap();
        bad.pricing["byteUsageFactor"]["denominator"] = 0.into();
        assert!(prepare(me, Some(&bad), None, &catalogue()).is_err());
        let mut bad = NodeConfig::default().offering.unwrap();
        bad.info["maximumRetentionSeconds"] = 0.into();
        assert!(prepare(me, Some(&bad), None, &catalogue()).is_err());
        let mut bad = NodeConfig::default().offering.unwrap();
        bad.pricing = Value::Null;
        let error = prepare(me, Some(&bad), None, &catalogue()).err().unwrap();
        assert!(!error.contains("null"));
    }
}
