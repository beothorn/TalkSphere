//! Remote definitions are immutable associations to a selected cache receipt, never nearby versions.
use super::{
    Error,
    authority::Action,
    cache::{Cache, Remote, key},
    contract::Context,
    definition::Definition,
};
use std::sync::Arc;
impl Cache {
    /// Attach a fully validated definition to the currently selected fresh Board receipt.
    /// # Errors
    /// Rejects stale receipt races, missing IDs, mismatches, invalid schemas and resource/control failures.
    pub fn attach(
        &mut self,
        context: &Context<'_>,
        remote: &Remote,
        id: &str,
        bytes: &[u8],
        now: u64,
    ) -> Result<Remote, Error> {
        tracing::trace!("Associating a complete definition with its selected remote receipt");
        let k = key(remote.provenance);
        context.check(&self.authority, k.0, Action::Cache, bytes.len())?;
        self.current(remote, now)?;
        let o = remote.board.offerings.get(id).ok_or(Error::Missing)?;
        let d = Definition::parse(bytes, o, self.limits)?;
        let mut next = remote.clone();
        next.definitions.insert(id.to_owned(), d);
        let total = next
            .definitions
            .values()
            .try_fold(next.bytes().len(), |n, d| {
                n.checked_add(d.document.bytes().len()).ok_or(Error::Limit)
            })?;
        if total > self.limits.publication_bytes {
            return Err(Error::Limit);
        }
        self.pins.retain(|p| p.strong_count() > 0);
        if self.pins.len() >= self.limits.snapshots {
            return Err(Error::Limit);
        }
        next.revision = Arc::new(());
        self.pins.push(Arc::downgrade(&next.revision));
        self.entries.insert(k, next.clone());
        Ok(next)
    }
    /// Read a definition from an immutable receipt under explicit Read authority.
    /// # Errors
    /// Rejects expired/stale/foreign receipts, missing IDs and caller authority failures.
    pub fn definition(
        &self,
        context: &Context<'_>,
        remote: &Remote,
        id: &str,
        now: u64,
    ) -> Result<Definition, Error> {
        tracing::trace!("Reading a validated cached definition");
        let k = key(remote.provenance);
        context.check(&self.authority, k.0, Action::Read, 0)?;
        self.current(remote, now)?;
        let d = remote.definitions.get(id).ok_or(Error::Missing)?;
        context.check(&self.authority, k.0, Action::Read, d.document.bytes().len())?;
        Ok(d.clone())
    }
    // Check exact current receipt identity so delayed definitions cannot attach to a replacement Board.
    pub(super) fn current(&self, remote: &Remote, now: u64) -> Result<(), Error> {
        tracing::trace!("Checking the selected cache receipt is current and fresh");
        let current = self
            .entries
            .get(&key(remote.provenance))
            .ok_or(Error::Missing)?;
        if !Arc::ptr_eq(&current.revision, &remote.revision)
            || now < remote.received
            || now >= remote.expires
        {
            return Err(Error::Stale);
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::{
        board::{authority::Authority, cache::Provenance, config::Limits, test_support},
        identity::api::IdentityId,
    };
    use std::time::{Duration, Instant};
    // Delayed replies cannot combine one publication's offering with another publication's definition.
    #[test]
    fn definition_receipts_never_mix_revisions() {
        let a = Arc::new(Authority::new());
        let id = IdentityId([1; 32]);
        let mut cache = Cache::new(a.clone(), Limits::default()).unwrap();
        let g = a.grant(id, Action::Cache, 10000).unwrap();
        let c = Context {
            grant: &g,
            deadline: Instant::now() + Duration::from_secs(10),
            cancelled: false,
        };
        let board = test_support::board(false, 7);
        let first = cache
            .store(&c, Provenance::Direct(id), board.as_bytes(), 100)
            .unwrap();
        let with_def = cache
            .attach(
                &c,
                &first,
                "fixture",
                test_support::definition().as_bytes(),
                100,
            )
            .unwrap();
        assert_eq!(first.definition_count(), 0);
        assert_eq!(with_def.definition_count(), 1);
        let read = a.grant(id, Action::Read, 10000).unwrap();
        let rc = Context { grant: &read, ..c };
        assert!(cache.definition(&rc, &with_def, "fixture", 101).is_ok());
        let second = cache
            .store(&c, Provenance::Direct(id), board.as_bytes(), 102)
            .unwrap();
        assert!(matches!(
            cache.attach(
                &c,
                &with_def,
                "fixture",
                test_support::definition().as_bytes(),
                103
            ),
            Err(Error::Stale)
        ));
        assert!(matches!(
            cache.definition(&rc, &second, "fixture", 103),
            Err(Error::Missing)
        ));
        assert!(matches!(
            cache.attach(&c, &second, "missing", b"{}", 103),
            Err(Error::Missing)
        ));
        assert!(cache.attach(&c, &second, "fixture", b"{}", 103).is_err());
    }
}
