//! Atomic complete-image transactions supplied by the local platform, never paid Storage.
use super::Error;
use crate::modules::identity::api::IdentityId;

/// Complete identity-bound local state image; payload schema is validated by the owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// Authoritative local account identity, never inferred from the file path.
    pub identity: IdentityId,
    /// Complete bounded state, including holds, effects and lifetime replay records.
    pub payload: Vec<u8>,
}
/// Exclusive-writer atomic persistence. Implementations must not acknowledge partial state.
pub trait Persistence: Send {
    /// Load complete committed bytes, synchronizing possibly installed state before recovery.
    /// # Errors
    /// Storage/Invalid/Limit reject unavailable/corrupt/oversized state, not absence.
    fn load(&mut self) -> Result<Option<Record>, Error>;
    /// Replace the entire image; success means bytes and required directory metadata are durable.
    /// # Errors
    /// Storage means no new commitment; Unknown means commitment is possible and requires reload.
    fn commit(&mut self, record: &Record) -> Result<(), Error>;
}
/// Explicitly volatile adapter for embedded experiments; provides no restart durability guarantee.
#[derive(Default)]
pub struct Memory(pub Option<Record>);
impl Persistence for Memory {
    // Return only a complete independent copy, never a borrowed mutable database.
    fn load(&mut self) -> Result<Option<Record>, Error> {
        tracing::trace!("Loading volatile Services image");
        Ok(self.0.clone())
    }
    // Replace a complete image without pretending to synchronize a filesystem.
    fn commit(&mut self, record: &Record) -> Result<(), Error> {
        tracing::trace!("Replacing volatile Services image");
        self.0 = Some(record.clone());
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    // Copies stay immutable across replacement and missing differs from an empty state image.
    #[test]
    fn complete_image_copy() {
        let mut m = Memory::default();
        assert_eq!(m.load(), Ok(None));
        let r = Record {
            identity: IdentityId([1; 32]),
            payload: vec![7],
        };
        m.commit(&r).unwrap();
        let copy = m.load().unwrap();
        m.commit(&Record {
            payload: vec![9],
            ..r.clone()
        })
        .unwrap();
        assert_eq!(copy, Some(r));
    }
}
