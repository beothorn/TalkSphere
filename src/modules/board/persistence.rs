//! Durable Board commit contract; local persistence is independent of the paid Storage service.
use super::{Error, config::Limits, prepared::Prepared, semantics::Capabilities};
use crate::modules::identity::api::IdentityId;
use std::collections::BTreeMap;

/// Complete stored publication associated with exactly one local publisher.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    /// Exact publisher identifier, not a path or pointer.
    pub publisher: IdentityId,
    /// Original Board JSON.
    pub board: Vec<u8>,
    /// Original matching definition JSON indexed by offering ID.
    pub definitions: BTreeMap<String, Vec<u8>>,
}
impl Record {
    /// Copy complete validated publication bytes for the platform commit boundary.
    pub fn from_prepared(p: &Prepared) -> Self {
        tracing::trace!("Copying a complete publication for persistence");
        Self {
            publisher: p.publisher,
            board: p.board.document.bytes().to_vec(),
            definitions: p
                .definitions
                .iter()
                .map(|(id, d)| (id.clone(), d.document.bytes().to_vec()))
                .collect(),
        }
    }
    /// Revalidate loaded durable bytes before exposing them through live publication.
    /// # Errors
    /// Returns all document/capability errors; durable bytes cannot bypass preparation.
    pub fn prepare(&self, limits: Limits, caps: &dyn Capabilities) -> Result<Prepared, Error> {
        tracing::trace!("Revalidating a recovered durable publication");
        Prepared::new(self.publisher, &self.board, &self.definitions, limits, caps)
    }
}
/// Atomic platform storage; acknowledgement means complete state survives supported restart.
/// Storage is a known precommit failure; Unknown means recover/load before further mutations.
pub trait Persistence: Send {
    /// Load the complete last committed record; no record is distinct from corruption.
    /// # Errors
    /// Returns Storage/Invalid/Unknown for I/O, corruption or unresolved durability.
    fn load(&mut self) -> Result<Option<Record>, Error>;
    /// Commit exactly one complete replacement and acknowledge only after durable synchronization.
    /// # Errors
    /// Storage preserves the prior publication; Unknown requires recovery and forbids assumed rollback.
    fn commit(&mut self, record: &Record) -> Result<(), Error>;
}
/// Explicit volatile adapter for embedded tests and ephemeral composition; no durability claim.
#[derive(Default)]
pub struct Memory(Option<Record>);
impl Persistence for Memory {
    // Return an owned complete copy without changing volatile state.
    fn load(&mut self) -> Result<Option<Record>, Error> {
        tracing::trace!("Loading a volatile Board record");
        Ok(self.0.clone())
    }
    // Atomic in-memory replacement; process restart loses this adapter's state.
    fn commit(&mut self, record: &Record) -> Result<(), Error> {
        tracing::trace!("Replacing a volatile Board record");
        self.0 = Some(record.clone());
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::super::semantics::Unavailable;
    use super::*;
    // Complete records roundtrip through the declared port; malformed recovered content cannot become live.
    #[test]
    fn record_revalidation_and_memory_contract() {
        let p = Prepared::new(
            IdentityId([1; 32]),
            br#"{"version":1,"providedServices":[],"wantedServices":[]}"#,
            &BTreeMap::new(),
            Limits::default(),
            &Unavailable,
        )
        .unwrap();
        let r = Record::from_prepared(&p);
        let mut m = Memory::default();
        assert_eq!(m.load(), Ok(None));
        m.commit(&r).unwrap();
        assert_eq!(m.load(), Ok(Some(r.clone())));
        assert!(r.prepare(Limits::default(), &Unavailable).is_ok());
        let mut bad = r;
        bad.board = b"corrupt".to_vec();
        assert!(bad.prepare(Limits::default(), &Unavailable).is_err());
    }
}
