//! Bounded remote Board cache with trusted provenance, immutable receipts and no hidden fetches.
use super::{
    Error,
    authority::{Action, Authority},
    config::Limits,
    contract::Context,
    offering::Board,
};
use crate::modules::identity::api::IdentityId;
use std::{collections::BTreeMap, sync::Arc};

/// Attribution supplied by a trusted local adapter, never extracted from document fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provenance {
    /// Directly received from this authenticated publisher.
    Direct(IdentityId),
    /// Forwarded by this peer; original publisher remains unauthenticated.
    Forwarded(IdentityId),
}
/// Immutable cache receipt; pinning does not make it fresh forever.
#[derive(Clone)]
pub struct Remote {
    pub(super) board: Arc<Board>,
    pub(super) provenance: Provenance,
    pub(super) received: u64,
    pub(super) expires: u64,
    pub(super) revision: Arc<()>,
    pub(super) definitions: BTreeMap<String, super::definition::Definition>,
}
impl Remote {
    /// Borrow exact source bytes, including unknown metadata, without implying freshness.
    pub fn bytes(&self) -> &[u8] {
        tracing::trace!("Reading pinned remote Board bytes");
        self.board.document.bytes()
    }
    /// Return the number of validated definitions associated with this immutable receipt.
    pub fn definition_count(&self) -> usize {
        tracing::trace!("Counting cached validated definitions");
        self.definitions.len()
    }
    /// Return trusted retrieval attribution; forwarded bytes never authenticate an original publisher.
    pub fn provenance(&self) -> Provenance {
        tracing::trace!("Reading remote Board provenance");
        self.provenance
    }
    /// Return receipt and expiry in the injected monotonic clock's seconds.
    pub fn times(&self) -> (u64, u64) {
        tracing::trace!("Reading bounded cache lifetime");
        (self.received, self.expires)
    }
}
/// Cache owner for trusted adapters; keyed by peer identity and direct/forwarded association.
pub struct Cache {
    pub(super) authority: Arc<Authority>,
    pub(super) limits: Limits,
    pub(super) entries: BTreeMap<(IdentityId, bool), Remote>,
    pub(super) pins: Vec<std::sync::Weak<()>>,
}
impl Cache {
    /// Create an empty cache without fetching data or invoking optional dependencies.
    /// # Errors
    /// Rejects invalid finite policy.
    pub fn new(authority: Arc<Authority>, limits: Limits) -> Result<Self, Error> {
        tracing::trace!("Creating a bounded remote Board cache");
        limits.validate()?;
        Ok(Self {
            authority,
            limits,
            entries: BTreeMap::new(),
            pins: Vec::new(),
        })
    }
    /// Validate and install a trusted direct or forwarded receipt atomically.
    /// # Errors
    /// Rejects unauthorized provenance assertions, invalid bytes, capacity and clock overflow.
    pub fn store(
        &mut self,
        context: &Context<'_>,
        provenance: Provenance,
        bytes: &[u8],
        now: u64,
    ) -> Result<Remote, Error> {
        tracing::trace!("Installing a complete attributed Board cache receipt");
        let key = key(provenance);
        context.check(&self.authority, key.0, Action::Cache, bytes.len())?;
        let board = Board::parse(bytes, self.limits)?;
        let expires = now
            .checked_add(self.limits.cache_seconds)
            .ok_or(Error::Limit)?;
        context.check(&self.authority, key.0, Action::Cache, bytes.len())?;
        self.entries.retain(|_, e| e.expires > now);
        self.pins.retain(|p| p.strong_count() > 0);
        let releases = self
            .entries
            .get(&key)
            .is_some_and(|e| Arc::strong_count(&e.revision) == 1);
        if (!self.entries.contains_key(&key) && self.entries.len() >= self.limits.cache_entries)
            || self.pins.len().saturating_sub(usize::from(releases)) >= self.limits.snapshots
        {
            return Err(Error::Limit);
        }
        let remote = Remote {
            board: Arc::new(board),
            provenance,
            received: now,
            expires,
            revision: Arc::new(()),
            definitions: BTreeMap::new(),
        };
        self.pins.push(Arc::downgrade(&remote.revision));
        self.entries.insert(key, remote.clone());
        Ok(remote)
    }
    /// Read a fresh receipt using local Read authority; expired/backward-time receipts are stale.
    /// # Errors
    /// Returns Missing/Stale or caller-control errors; never refreshes over the network.
    pub fn read(
        &self,
        context: &Context<'_>,
        provenance: Provenance,
        now: u64,
    ) -> Result<Remote, Error> {
        tracing::trace!("Reading a fresh attributed cache entry");
        let key = key(provenance);
        context.check(&self.authority, key.0, Action::Read, 0)?;
        let remote = self.entries.get(&key).ok_or(Error::Missing)?;
        if now < remote.received || now >= remote.expires {
            return Err(Error::Stale);
        }
        context.check(&self.authority, key.0, Action::Read, remote.bytes().len())?;
        Ok(remote.clone())
    }
}
// Direct and forwarded receipts never share attribution even when the byte strings are identical.
pub(super) fn key(p: Provenance) -> (IdentityId, bool) {
    tracing::trace!("Selecting the cache provenance key");
    match p {
        Provenance::Direct(id) => (id, true),
        Provenance::Forwarded(id) => (id, false),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    // Identical forwarded bytes do not gain original authentication; expiry and capacity are explicit.
    #[test]
    fn provenance_expiry_and_capacity() {
        let a = Arc::new(Authority::new());
        let id = IdentityId([1; 32]);
        let l = Limits {
            cache_entries: 1,
            ..Limits::default()
        };
        let mut cache = Cache::new(a.clone(), l).unwrap();
        let g = a.grant(id, Action::Cache, 1000).unwrap();
        let c = Context {
            grant: &g,
            deadline: Instant::now() + Duration::from_secs(10),
            cancelled: false,
        };
        let bytes = br#"{"version":1,"providedServices":[],"wantedServices":[]}"#;
        let old = cache.store(&c, Provenance::Direct(id), bytes, 100).unwrap();
        assert_eq!(old.provenance(), Provenance::Direct(id));
        assert_eq!(old.times(), (100, 400));
        assert!(matches!(
            cache.store(&c, Provenance::Forwarded(id), bytes, 100),
            Err(Error::Limit)
        ));
        let read = a.grant(id, Action::Read, 1000).unwrap();
        let rc = Context { grant: &read, ..c };
        assert!(cache.read(&rc, Provenance::Direct(id), 101).is_ok());
        assert!(matches!(
            cache.read(&rc, Provenance::Direct(id), 400),
            Err(Error::Stale)
        ));
        let forwarded = cache
            .store(&c, Provenance::Forwarded(id), bytes, 401)
            .unwrap();
        assert_eq!(forwarded.provenance(), Provenance::Forwarded(id));
        assert_eq!(old.bytes(), forwarded.bytes());
        assert!(matches!(
            cache.store(&c, Provenance::Direct(id), bytes, u64::MAX),
            Err(Error::Limit)
        ));
    }
    // An unpinned current cache entry can be replaced within a one-revision budget.
    #[test]
    fn unpinned_cache_revision_is_reclaimable() {
        let a = Arc::new(Authority::new());
        let id = IdentityId([1; 32]);
        let mut cache = Cache::new(
            a.clone(),
            Limits {
                snapshots: 1,
                ..Limits::default()
            },
        )
        .unwrap();
        let g = a.grant(id, Action::Cache, 1000).unwrap();
        let c = Context {
            grant: &g,
            deadline: Instant::now() + Duration::from_secs(10),
            cancelled: false,
        };
        let raw = br#"{"version":1,"providedServices":[],"wantedServices":[]}"#;
        let old = cache.store(&c, Provenance::Direct(id), raw, 100).unwrap();
        assert!(matches!(
            cache.store(&c, Provenance::Direct(id), raw, 101),
            Err(Error::Limit)
        ));
        drop(old);
        assert!(cache.store(&c, Provenance::Direct(id), raw, 102).is_ok());
    }
}
