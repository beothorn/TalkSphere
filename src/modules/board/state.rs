//! Atomic local Board ownership; immutable pins never mix offering and definition revisions.
use super::{
    Error,
    authority::{Action, Authority},
    config::Limits,
    contract::{Context, Snapshot},
    definition::Definition,
    prepared::Prepared,
    semantics::Capabilities,
};
use crate::modules::identity::api::IdentityId;
use std::sync::{Arc, Mutex, Weak};

struct State {
    storage: Box<dyn super::persistence::Persistence>,
    uncertain: bool,
    current: Option<Snapshot>,
    pins: Vec<Weak<Prepared>>,
}
/// Direct-call owner for one publisher, with finite pin retention and serialized writers.
///
/// This example publishes an empty volatile Board; composition supplies its actual local identity.
/// No service handler, network authentication or spending capability is created.
/// ```
/// use std::{collections::BTreeMap,sync::Arc,time::{Duration,Instant}};
/// use talksphere::modules::{identity::api::IdentityId,board::{
///     authority::{Authority,Action},config::Limits,contract::Context,
///     prepared::Prepared,semantics::Unavailable,state::Provider,
/// }};
/// let local=IdentityId([1;32]); // Illustrative public ID; obtain the real ID from Identity in composition.
/// let limits=Limits::default();
/// let authority=Arc::new(Authority::new());
/// let grant=authority.grant(local,Action::Publish,limits.publication_bytes)?;
/// let context=Context {grant:&grant,deadline:Instant::now()+Duration::from_secs(10),cancelled:false};
/// let candidate=Prepared::new(local,br#"{"version":1,"providedServices":[],"wantedServices":[]}"#,
///     &BTreeMap::new(),limits,&Unavailable)?;
/// let owner=Provider::new(local,authority,limits,Arc::new(Unavailable))?;
/// let snapshot=owner.publish(&context,candidate)?;
/// assert_eq!(snapshot.publisher(),local);
/// # Ok::<(),talksphere::modules::board::Error>(())
/// ```
pub struct Provider {
    pub(super) authority: Arc<Authority>,
    pub(super) limits: Limits,
    pub(super) publisher: IdentityId,
    pub(super) caps: Arc<dyn Capabilities>,
    domain: Arc<()>,
    state: Mutex<State>,
}
impl Provider {
    /// Create an empty volatile owner; no publication exists until an authorized complete commit.
    /// # Errors
    /// Rejects invalid limits before creating state or effects.
    pub fn new(
        publisher: IdentityId,
        authority: Arc<Authority>,
        limits: Limits,
        caps: Arc<dyn Capabilities>,
    ) -> Result<Self, Error> {
        tracing::trace!("Creating an atomic Board publication owner");
        limits.validate()?;
        Ok(Self {
            authority,
            limits,
            publisher,
            caps,
            domain: Arc::new(()),
            state: Mutex::new(State {
                storage: Box::<super::persistence::Memory>::default(),
                uncertain: false,
                current: None,
                pins: Vec::new(),
            }),
        })
    }
    /// Open a declared persistence adapter and recover complete validated state before serving reads.
    /// # Errors
    /// Rejects loaded corruption, wrong publisher, unavailable semantics and unresolved storage outcomes.
    pub fn open(
        publisher: IdentityId,
        authority: Arc<Authority>,
        limits: Limits,
        caps: Arc<dyn Capabilities>,
        storage: Box<dyn super::persistence::Persistence>,
    ) -> Result<Self, Error> {
        tracing::trace!("Recovering a Board owner from its declared persistence port");
        let mut owner = Self::new(publisher, authority, limits, caps)?;
        let state = owner.state.get_mut().map_err(|_| Error::Unknown)?;
        state.storage = storage;
        if let Some(record) = state.storage.load()? {
            if record.publisher != publisher {
                return Err(Error::Denied);
            }
            let data = Arc::new(record.prepare(limits, owner.caps.as_ref())?);
            let snapshot = Snapshot {
                publisher,
                board: Arc::from(data.board.document.bytes()),
                data: Some(data.clone()),
                domain: Arc::downgrade(&owner.domain),
            };
            state.pins.push(Arc::downgrade(&data));
            state.current = Some(snapshot);
        }
        Ok(owner)
    }
    /// Reconcile a possibly committed write under Publish authority without replaying that write.
    /// # Errors
    /// Returns Unknown until a complete readable record is recovered; wrong publisher/corruption fail explicitly.
    pub fn recover(&self, context: &Context<'_>) -> Result<(), Error> {
        tracing::trace!("Reconciling Board durability without automatic republication");
        context.check(&self.authority, self.publisher, Action::Publish, 0)?;
        let mut state = self.state.lock().map_err(|_| Error::Unknown)?;
        let record = state.storage.load()?;
        let current = if let Some(record) = record {
            if record.publisher != self.publisher {
                return Err(Error::Denied);
            }
            let data = Arc::new(record.prepare(self.limits, self.caps.as_ref())?);
            state.pins.retain(|pin| pin.strong_count() > 0);
            // Replacing an unpinned current revision releases that owner's only strong reference.
            let releases_current = state
                .current
                .as_ref()
                .and_then(|s| s.data.as_ref())
                .is_some_and(|p| Arc::strong_count(p) == 1);
            if state
                .pins
                .len()
                .saturating_sub(usize::from(releases_current))
                >= self.limits.snapshots
            {
                return Err(Error::Limit);
            }
            state.pins.push(Arc::downgrade(&data));
            Some(Snapshot {
                publisher: self.publisher,
                board: Arc::from(data.board.document.bytes()),
                data: Some(data),
                domain: Arc::downgrade(&self.domain),
            })
        } else {
            None
        };
        context.check(&self.authority, self.publisher, Action::Publish, 0)?;
        state.current = current;
        state.uncertain = false;
        Ok(())
    }
    /// Replace the complete publication after authority and current capability checks.
    /// # Errors
    /// Rejects wrong publisher, revoked grants, limits and unavailable semantics without replacing state.
    pub fn publish(&self, context: &Context<'_>, candidate: Prepared) -> Result<Snapshot, Error> {
        tracing::trace!("Atomically replacing a complete Board snapshot");
        context.check(
            &self.authority,
            self.publisher,
            Action::Publish,
            candidate.bytes(),
        )?;
        if candidate.publisher() != self.publisher {
            return Err(Error::Denied);
        }
        if candidate.bytes() > self.limits.publication_bytes {
            return Err(Error::Limit);
        }
        let definitions = candidate
            .definitions
            .iter()
            .map(|(id, d)| (id.clone(), d.document.bytes().to_vec()))
            .collect();
        let candidate = Prepared::new(
            self.publisher,
            candidate.board.document.bytes(),
            &definitions,
            self.limits,
            self.caps.as_ref(),
        )?;
        let mut state = self.state.lock().map_err(|_| Error::Unknown)?;
        context.check(
            &self.authority,
            self.publisher,
            Action::Publish,
            candidate.bytes(),
        )?;
        if state.uncertain {
            return Err(Error::Unknown);
        }
        state.pins.retain(|pin| pin.strong_count() > 0);
        // Replacing an unpinned current revision releases that owner's only strong reference.
        let releases_current = state
            .current
            .as_ref()
            .and_then(|s| s.data.as_ref())
            .is_some_and(|p| Arc::strong_count(p) == 1);
        if state
            .pins
            .len()
            .saturating_sub(usize::from(releases_current))
            >= self.limits.snapshots
        {
            return Err(Error::Limit);
        }
        candidate.assess(self.caps.as_ref())?;
        context.check(
            &self.authority,
            self.publisher,
            Action::Publish,
            candidate.bytes(),
        )?;
        if let Err(error) = state
            .storage
            .commit(&super::persistence::Record::from_prepared(&candidate))
        {
            if error == Error::Unknown {
                state.uncertain = true;
            }
            return Err(error);
        }
        let data = Arc::new(candidate);
        let snapshot = Snapshot {
            publisher: self.publisher,
            board: Arc::from(data.board.document.bytes()),
            data: Some(data.clone()),
            domain: Arc::downgrade(&self.domain),
        };
        state.pins.push(Arc::downgrade(&data));
        state.current = Some(snapshot.clone());
        Ok(snapshot)
    }
    /// Pin the complete current revision under Read authority; initial empty state is explicit Missing.
    /// # Errors
    /// Returns Denied/Cancelled/Limit for control failures, Missing before publication, Unknown on poisoned ownership.
    pub fn read(&self, context: &Context<'_>) -> Result<Snapshot, Error> {
        tracing::trace!("Pinning one consistent local Board revision");
        context.check(&self.authority, self.publisher, Action::Read, 0)?;
        let state = self.state.lock().map_err(|_| Error::Unknown)?;
        if state.uncertain {
            return Err(Error::Unknown);
        }
        let snapshot = state.current.as_ref().ok_or(Error::Missing)?;
        context.check(
            &self.authority,
            self.publisher,
            Action::Read,
            snapshot.bytes().len(),
        )?;
        Ok(snapshot.clone())
    }
    /// Retrieve a definition from exactly the supplied owner-issued revision.
    /// # Errors
    /// Rejects foreign pins and authority failures; a missing ID returns Missing without substitution.
    pub fn definition(
        &self,
        context: &Context<'_>,
        snapshot: &Snapshot,
        id: &str,
    ) -> Result<Definition, Error> {
        tracing::trace!("Reading a definition from a pinned Board revision");
        self.check_snapshot(snapshot)?;
        let d = snapshot
            .data
            .as_ref()
            .ok_or(Error::Missing)?
            .definitions
            .get(id)
            .ok_or(Error::Missing)?;
        context.check(
            &self.authority,
            self.publisher,
            Action::Read,
            d.document.bytes().len(),
        )?;
        Ok(d.clone())
    }
    /// Verify selected terms still exactly match the current offering and definition.
    /// # Errors
    /// Returns Stale after removal/repricing/contract change and Unknown during unresolved durability.
    pub fn current_selection(
        &self,
        context: &Context<'_>,
        selected: &super::selection::Selection,
    ) -> Result<(), Error> {
        tracing::trace!("Checking current terms without changing retained selections");
        context.check(
            &self.authority,
            self.publisher,
            Action::Select,
            selected.offering().source.to_string().len(),
        )?;
        if selected.publisher() != self.publisher {
            return Err(Error::Denied);
        }
        let state = self.state.lock().map_err(|_| Error::Unknown)?;
        if state.uncertain {
            return Err(Error::Unknown);
        }
        let data = state
            .current
            .as_ref()
            .and_then(|s| s.data.as_ref())
            .ok_or(Error::Stale)?;
        let id = &selected.offering().id;
        if data
            .board
            .offerings
            .get(id)
            .is_none_or(|o| o.source != selected.offering().source)
            || data
                .definitions
                .get(id)
                .is_none_or(|d| d.document.value() != selected.definition().document.value())
        {
            return Err(Error::Stale);
        }
        Ok(())
    }
    // A pin from another owner or a dropped owner never supplies local authority.
    pub(super) fn check_snapshot(&self, snapshot: &Snapshot) -> Result<(), Error> {
        tracing::trace!("Checking Board snapshot owner association");
        let domain = snapshot.domain.upgrade().ok_or(Error::Denied)?;
        if !Arc::ptr_eq(&domain, &self.domain) || snapshot.publisher() != self.publisher {
            return Err(Error::Denied);
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::super::semantics::Unavailable;
    use super::*;
    use std::{
        collections::BTreeMap,
        time::{Duration, Instant},
    };
    // Pins retain complete immutable content; foreign owner references and capacity exhaustion are rejected.
    #[test]
    fn immutable_atomic_revisions_and_owner_scope() {
        let id = IdentityId([1; 32]);
        let authority = Arc::new(Authority::new());
        let l = Limits {
            snapshots: 2,
            ..Limits::default()
        };
        let p = Provider::new(id, authority.clone(), l, Arc::new(Unavailable)).unwrap();
        let grant = authority
            .grant(id, Action::Publish, l.publication_bytes)
            .unwrap();
        let c = Context {
            grant: &grant,
            deadline: Instant::now() + Duration::from_secs(10),
            cancelled: false,
        };
        let empty = br#"{"version":1,"providedServices":[],"wantedServices":[]}"#;
        let make = || Prepared::new(id, empty, &BTreeMap::new(), l, &Unavailable).unwrap();
        let first = p.publish(&c, make()).unwrap();
        let second = p.publish(&c, make()).unwrap();
        assert_eq!(first.bytes(), second.bytes());
        assert!(matches!(p.publish(&c, make()), Err(Error::Limit)));
        drop(first);
        assert!(p.publish(&c, make()).is_ok());
        let read = authority
            .grant(id, Action::Read, l.publication_bytes)
            .unwrap();
        let rc = Context { grant: &read, ..c };
        assert!(p.read(&rc).is_ok());
        assert!(matches!(
            p.definition(&rc, &second, "absent"),
            Err(Error::Missing)
        ));
        let other = Provider::new(id, authority, l, Arc::new(Unavailable)).unwrap();
        assert!(matches!(
            other.definition(&rc, &second, "absent"),
            Err(Error::Denied)
        ));
    }
}
