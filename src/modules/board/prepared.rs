//! Complete immutable publication candidates; validation never mutates published state.
use super::{
    Error,
    config::Limits,
    definition::Definition,
    offering::Board,
    response,
    semantics::{self, Capabilities},
};
use crate::modules::identity::api::IdentityId;
use std::collections::BTreeMap;

/// Complete validated Board and matching definitions, constructible only through preparation.
#[derive(Clone, Debug)]
pub struct Prepared {
    pub(super) publisher: IdentityId,
    pub(super) board: Board,
    pub(super) definitions: BTreeMap<String, Definition>,
    pub(super) bytes: usize,
}
impl Prepared {
    /// Validate a whole publication without changing state, registering handlers or writing storage.
    /// # Errors
    /// Rejects missing/extra/mismatched definitions, invalid semantics, inconsistent contracts and limits.
    pub fn new(
        publisher: IdentityId,
        board: &[u8],
        definitions: &BTreeMap<String, Vec<u8>>,
        limits: Limits,
        caps: &dyn Capabilities,
    ) -> Result<Self, Error> {
        tracing::trace!("Preparing a complete Board publication without effects");
        limits.validate()?;
        if definitions.len() > limits.definitions {
            return Err(Error::Limit);
        }
        let bytes = definitions.values().try_fold(board.len(), |sum, d| {
            sum.checked_add(d.len()).ok_or(Error::Limit)
        })?;
        if bytes > limits.publication_bytes {
            return Err(Error::Limit);
        }
        let board = Board::parse(board, limits)?;
        response::encode("board", &board.document)?;
        if definitions.len() != board.offerings.len() {
            return Err(Error::Invalid);
        }
        let mut validated = BTreeMap::new();
        let mut contracts: BTreeMap<(String, u32), String> = BTreeMap::new();
        for (id, o) in &board.offerings {
            let d = Definition::parse(definitions.get(id).ok_or(Error::Missing)?, o, limits)?;
            response::encode("definition", &d.document)?;
            let contract = (o.service.clone(), o.version);
            if let Some(prior_id) = contracts.get(&contract) {
                let prior = validated.get(prior_id).ok_or(Error::Invalid)?;
                if !d.same_contract(prior) {
                    return Err(Error::Invalid);
                }
            } else {
                contracts.insert(contract, id.clone());
            }
            validated.insert(id.clone(), d);
        }
        let result = Self {
            publisher,
            board,
            definitions: validated,
            bytes,
        };
        result.assess(caps)?;
        Ok(result)
    }
    /// Recheck current local semantic capabilities before committing an already prepared candidate.
    /// # Errors
    /// Rejects withdrawn handlers/requester support, unknown semantics and invalid pricing terms.
    pub fn assess(&self, caps: &dyn Capabilities) -> Result<(), Error> {
        tracing::trace!("Rechecking publication capabilities before visibility");
        for (id, o) in &self.board.offerings {
            semantics::assess(caps, o, &self.definitions[id], true, true)?;
        }
        Ok(())
    }
    /// Return exact publisher identity associated with the candidate.
    pub fn publisher(&self) -> IdentityId {
        tracing::trace!("Reading prepared publisher identity");
        self.publisher
    }
    /// Return total input bytes charged against publication and authority budgets.
    pub fn bytes(&self) -> usize {
        tracing::trace!("Reading prepared publication size");
        self.bytes
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::board::{semantics::Unavailable, test_support};
    // Empty snapshots need no fake handler; invalid or incomplete candidates never become prepared.
    #[test]
    fn complete_candidates_only() {
        let id = IdentityId([1; 32]);
        let limits = Limits::default();
        let empty = br#"{"version":1,"providedServices":[],"wantedServices":[]}"#;
        let p = Prepared::new(id, empty, &BTreeMap::new(), limits, &Unavailable).unwrap();
        assert_eq!(p.publisher(), id);
        assert_eq!(p.bytes(), empty.len());
        let raw = test_support::board(false, 7);
        assert!(Prepared::new(id, raw.as_bytes(), &BTreeMap::new(), limits, &Unavailable).is_err());
        let defs = BTreeMap::from([(
            "fixture".into(),
            test_support::definition().as_bytes().to_vec(),
        )]);
        assert!(matches!(
            Prepared::new(id, raw.as_bytes(), &defs, limits, &Unavailable),
            Err(Error::Unsupported)
        ));
        assert!(Prepared::new(id, empty, &defs, limits, &Unavailable).is_err());
    }
}

#[cfg(test)]
mod consistency_tests {
    use super::*;
    // Shared service/version requires a single full logical contract despite distinct offering IDs.
    #[test]
    fn conflicting_same_version_definitions_are_rejected() {
        let mut b: serde_json::Value =
            serde_json::from_str(&super::super::test_support::board(false, 7)).unwrap();
        let mut o = b["providedServices"][0].clone();
        o["id"] = "second".into();
        b["providedServices"].as_array_mut().unwrap().push(o);
        let mut d: serde_json::Value =
            serde_json::from_str(super::super::test_support::definition()).unwrap();
        d["id"] = "second".into();
        d["input"]["type"] = "bool".into();
        let mut defs = BTreeMap::from([
            (
                "fixture".into(),
                super::super::test_support::definition().as_bytes().to_vec(),
            ),
            ("second".into(), d.to_string().into_bytes()),
        ]);
        assert!(matches!(
            Prepared::new(
                IdentityId([1; 32]),
                b.to_string().as_bytes(),
                &defs,
                Limits::default(),
                &super::super::scenarios::Known
            ),
            Err(Error::Invalid)
        ));
        d["input"]["type"] = "string".into();
        d["serviceVersion"] = serde_json::from_str("1.0").unwrap();
        defs.insert("second".into(), d.to_string().into_bytes());
        assert!(
            Prepared::new(
                IdentityId([1; 32]),
                b.to_string().as_bytes(),
                &defs,
                Limits::default(),
                &super::super::scenarios::Known
            )
            .is_ok()
        );
    }
}
