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
        for (id, o) in &board.offerings {
            let d = Definition::parse(definitions.get(id).ok_or(Error::Missing)?, o, limits)?;
            response::encode("definition", &d.document)?;
            for (other_id, other) in &board.offerings {
                if other_id >= id {
                    break;
                }
                if other.service == o.service && other.version == o.version {
                    let prior = validated.get(other_id).ok_or(Error::Invalid)?;
                    if !d.same_contract(prior) {
                        return Err(Error::Invalid);
                    }
                }
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
