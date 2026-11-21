//! Builds the node's Storage offering and its independently recognized definition.
//!
//! Callers: owner opening. The terms come only from operator configuration; the Board
//! parser and the Storage standards catalogue validate them, so unrepresentable prices or
//! maxima are refused before anything is published.
use super::config::Terms;
use crate::modules::{
    board::{definition::Definition, offering::Board},
    identity::{api::IdentityId, text},
    storage::standard::Catalogue,
};
use serde_json::{Value, json};

/// Identifier of the node's one Storage offering.
pub const OFFERING_ID: &str = "storage-standard";

/// A validated offering: the Board document bytes and its complete standard definition.
pub struct Prepared {
    /// Canonical Board document with one provided Storage offering, or none.
    pub board: Vec<u8>,
    /// The standard definition both requester and provider install.
    pub definition: Definition,
    /// Bytes of the definition document, published beside the Board.
    pub definition_bytes: Vec<u8>,
}

/// Build and validate the offering for `local`; `terms` of `None` yields an empty Board
/// plus the default definition, which a requester-only node still needs for its client.
/// # Errors
/// Terms the Board parser or the Storage catalogue cannot represent are refused with a
/// message naming the failing stage, never the values.
pub fn prepare(
    local: IdentityId,
    terms: Option<&Terms>,
    catalogue: &Catalogue,
) -> Result<Prepared, String> {
    tracing::trace!("Preparing the node Storage offering");
    let provided = offering(
        local,
        terms.unwrap_or(
            &super::config::NodeConfig::default()
                .offering
                .ok_or("no default offering")?,
        ),
    );
    // The definition always comes from a fully validated offering, even when none is published.
    let raw = serde_json::to_vec(&json!({
        "version": 1, "providedServices": [provided], "wantedServices": []
    }))
    .map_err(|_| "offering could not be encoded")?;
    let parsed =
        Board::parse(&raw, Default::default()).map_err(|_| "offering terms are invalid")?;
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
    let board = if terms.is_some() {
        raw
    } else {
        br#"{"version":1,"providedServices":[],"wantedServices":[]}"#.to_vec()
    };
    Ok(Prepared {
        board,
        definition,
        definition_bytes,
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
        let provided = prepare(me, config.offering.as_ref(), &catalogue()).unwrap();
        let text = String::from_utf8(provided.board).unwrap();
        assert!(text.contains("storage-standard") && text.contains(&super::text::format(me)));
        assert!(!provided.definition_bytes.is_empty());
        let none = prepare(me, None, &catalogue()).unwrap();
        assert!(String::from_utf8(none.board).unwrap().contains("[]"));
        assert!(!none.definition_bytes.is_empty());
    }
    // Unrepresentable prices and maxima are refused before publication, without echoing values.
    #[test]
    fn invalid_terms_are_refused() {
        let me = IdentityId([1; 32]);
        let mut bad = NodeConfig::default().offering.unwrap();
        bad.pricing["byteUsageFactor"]["denominator"] = 0.into();
        assert!(prepare(me, Some(&bad), &catalogue()).is_err());
        let mut bad = NodeConfig::default().offering.unwrap();
        bad.info["maximumRetentionSeconds"] = 0.into();
        assert!(prepare(me, Some(&bad), &catalogue()).is_err());
        let mut bad = NodeConfig::default().offering.unwrap();
        bad.pricing = Value::Null;
        let error = prepare(me, Some(&bad), &catalogue()).err().unwrap();
        assert!(!error.contains("null"));
    }
}
