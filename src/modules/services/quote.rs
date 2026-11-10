//! Stream-bound transient quotes; peer encoding is supplied by the trusted Peer Protocol adapter.
use super::{
    Error,
    assessment::Assessed,
    authority::{Action, Authority},
    contract::{Context, Limits, OperationKey},
    handler::{Blob, Peer},
    pricing::{self, Price},
};
use crate::modules::{
    board::{self, selection::Selection},
    identity::api::IdentityId,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, Weak},
    time::Instant,
};

/// Trusted monotonic clock dependency; it never reads a peer-supplied timestamp.
pub trait Clock: Send + Sync {
    /// Return the current local monotonic instant for expiry and reclamation.
    fn now(&self) -> Instant;
}
struct SystemClock;
impl Clock for SystemClock {
    // Production expiry follows the process monotonic clock.
    fn now(&self) -> Instant {
        tracing::trace!("Reading local quote expiry clock");
        Instant::now()
    }
}
/// Current-offering validation port; acceptance cannot assume copied terms remain current.
pub trait Current {
    /// Confirm current exact offering/definition and resource policy without changing accepted terms.
    /// # Errors
    /// Changed, removed, unsupported or uncertain state must reject new acceptance.
    fn check(&self, selection: &Selection) -> Result<(), Error>;
}
/// Public Board-owner adapter; caller supplies a valid Board Select context.
pub struct BoardCurrent<'a> {
    /// Authoritative local publication owner.
    pub owner: &'a board::state::Provider,
    /// Authorized current-selection call context.
    pub context: &'a board::contract::Context<'a>,
}
impl Current for BoardCurrent<'_> {
    // Compare the entire selected contract against the owner, never its private tables.
    fn check(&self, selection: &Selection) -> Result<(), Error> {
        tracing::trace!("Rechecking current Board terms at quote acceptance");
        self.owner
            .current_selection(self.context, selection)
            .map_err(|e| match e {
                board::Error::Denied => Error::Denied,
                board::Error::Limit => Error::Limit,
                board::Error::Cancelled => Error::Cancelled,
                board::Error::Unknown => Error::Unknown,
                board::Error::Storage => Error::Storage,
                board::Error::Unsupported => Error::Unsupported,
                board::Error::Invalid => Error::Invalid,
                board::Error::Missing | board::Error::Stale => Error::Conflict,
            })
    }
}
/// Exact quote and association from a locally authorized encoder; bytes must encode the assessed terms.
pub struct Issue {
    /// Pinned authorized provider assessment.
    pub assessed: Assessed,
    /// Exact complete quote JSON bytes from the trusted wire adapter, never a re-encoding on receipt.
    pub bytes: Blob,
    /// Explicit requester maximum, before any separately agreed issuer fee.
    pub maximum: u64,
    /// Local monotonic expiry for unaccepted quote resources.
    pub expires: Instant,
}
/// Opaque process-local quote handle, scoped to its issuing broker and original stream.
pub struct Quote {
    domain: Weak<()>,
    key: OperationKey,
    hash: [u8; 32],
    bytes: Blob,
    price: Price,
    generation: Arc<()>,
}
impl Quote {
    /// Read the immutable price captured when this exact handle was issued.
    pub fn price(&self) -> Price {
        tracing::trace!("Reading frozen issued quote price");
        self.price
    }
    /// Borrow exact quote bytes retained for hashing and transmission.
    pub fn bytes(&self) -> &[u8] {
        tracing::trace!("Borrowing exact quote bytes");
        self.bytes.bytes()
    }
    /// Return SHA-256 of exact retained quote bytes, not normalized JSON.
    pub fn hash(&self) -> [u8; 32] {
        tracing::trace!("Reading exact quote hash");
        self.hash
    }
}
/// Accepted immutable terms owned by the coordinator after stream-bound explicit acceptance.
pub struct Accepted {
    pub(crate) assessed: Assessed,
    pub(crate) price: Price,
    pub(crate) bytes: Blob,
    pub(crate) maximum: u64,
}
impl Accepted {
    /// Borrow the assessed exact selection and retained provider pin.
    pub fn assessed(&self) -> &Assessed {
        tracing::trace!("Borrowing accepted assessment");
        &self.assessed
    }
    /// Read immutable price authorization.
    pub fn price(&self) -> Price {
        tracing::trace!("Reading accepted price authorization");
        self.price
    }
    /// Borrow exact accepted quote bytes for durable operation binding.
    pub fn bytes(&self) -> &Blob {
        tracing::trace!("Borrowing exact accepted quote binding");
        &self.bytes
    }
    /// Read the requester's explicit spending ceiling, without adding an implicit fee.
    pub fn maximum(&self) -> u64 {
        tracing::trace!("Reading accepted requester bound");
        self.maximum
    }
}
struct Slot {
    generation: Arc<()>,
    issue: Issue,
    price: Price,
    hash: [u8; 32],
}
/// Bounded local quote owner; no quote alone reserves credits or permits execution.
pub struct Quoter {
    authority: Arc<Authority>,
    local: IdentityId,
    limits: Limits,
    domain: Arc<()>,
    slots: Mutex<BTreeMap<OperationKey, Slot>>,
    clock: Arc<dyn Clock>,
}
impl Quoter {
    /// Create an empty quote domain with finite capacity and identity-bound authority.
    /// # Errors
    /// Rejects invalid configured limits.
    pub fn new(
        local: IdentityId,
        authority: Arc<Authority>,
        limits: Limits,
    ) -> Result<Self, Error> {
        Self::with_clock(local, authority, limits, Arc::new(SystemClock))
    }
    /// Create a quote owner using an explicitly trusted local monotonic clock dependency.
    /// # Errors
    /// Rejects invalid finite limits; composition must supply a nondecreasing clock.
    pub fn with_clock(
        local: IdentityId,
        authority: Arc<Authority>,
        limits: Limits,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, Error> {
        tracing::trace!("Creating bounded Services quote owner");
        limits.validate()?;
        Ok(Self {
            authority,
            local,
            limits,
            domain: Arc::new(()),
            slots: Mutex::new(BTreeMap::new()),
            clock,
        })
    }
    /// Retain one exact quote after pricing and caller-bound admission; stale quotes are reclaimed first.
    /// # Errors
    /// Rejects empty/oversized bytes, wrong roles, authority, expiry, duplicate stream or full capacity.
    pub fn issue(&self, context: &Context<'_>, issue: Issue) -> Result<Quote, Error> {
        tracing::trace!("Issuing exact stream-bound service quote");
        let peer = issue.assessed.authorized.peer;
        if peer.local != self.local || issue.bytes.bytes().is_empty() {
            return Err(Error::Invalid);
        }
        if issue.bytes.bytes().len() > self.limits.state_bytes {
            return Err(Error::Limit);
        }
        let (requester, _provider, _) = issue.assessed.authorized.selection.roles();
        let action = if requester == self.local {
            Action::Request
        } else {
            Action::Invoke
        };
        context.check(
            &self.authority,
            self.local,
            action,
            issue.bytes.bytes().len(),
        )?;
        if issue.expires <= self.clock.now() {
            return Err(Error::Cancelled);
        }
        let price = pricing::quote(&issue.assessed, issue.maximum)?;
        let hash = Sha256::digest(issue.bytes.bytes()).into();
        let generation = Arc::new(());
        let q = Quote {
            domain: Arc::downgrade(&self.domain),
            key: peer.key,
            hash,
            bytes: issue.bytes.clone(),
            price,
            generation: generation.clone(),
        };
        let mut slots = self.slots.lock().map_err(|_| Error::Unavailable)?;
        context.check(
            &self.authority,
            self.local,
            action,
            issue.bytes.bytes().len(),
        )?;
        slots.retain(|_, slot| slot.issue.expires > self.clock.now());
        if slots.contains_key(&peer.key) {
            return Err(Error::Conflict);
        }
        if slots.len() >= self.limits.quotes {
            return Err(Error::Limit);
        }
        slots.insert(
            peer.key,
            Slot {
                generation,
                issue,
                price,
                hash,
            },
        );
        Ok(q)
    }
    /// Accept only the original exact quote on its trusted stream after current Board revalidation.
    /// # Errors
    /// Foreign handles/contexts, changed hashes, stale terms, cancellation and repeated acceptance fail.
    pub fn accept(
        &self,
        context: &Context<'_>,
        quote: &Quote,
        peer: Peer,
        hash: [u8; 32],
        current: &dyn Current,
    ) -> Result<Accepted, Error> {
        tracing::trace!("Accepting exact quote after current offering validation");
        let domain = quote.domain.upgrade().ok_or(Error::Denied)?;
        if !Arc::ptr_eq(&domain, &self.domain)
            || peer.local != self.local
            || peer.key != quote.key
            || hash != quote.hash
        {
            return Err(Error::Denied);
        }
        let mut slots = self.slots.lock().map_err(|_| Error::Unavailable)?;
        let s = slots.get(&quote.key).ok_or(Error::Conflict)?;
        if !Arc::ptr_eq(&s.generation, &quote.generation) {
            return Err(Error::Conflict);
        }
        let original = s.issue.assessed.authorized.peer;
        if original.remote != peer.remote || s.hash != hash {
            return Err(Error::Denied);
        }
        let (requester, _provider, _) = s.issue.assessed.authorized.selection.roles();
        let action = if requester == self.local {
            Action::Request
        } else {
            Action::Invoke
        };
        context.check(
            &self.authority,
            self.local,
            action,
            s.issue.bytes.bytes().len(),
        )?;
        if s.issue.expires <= self.clock.now() {
            return Err(Error::Cancelled);
        }
        current.check(&s.issue.assessed.authorized.selection)?;
        context.check(
            &self.authority,
            self.local,
            action,
            s.issue.bytes.bytes().len(),
        )?;
        if s.issue.expires <= self.clock.now() {
            return Err(Error::Cancelled);
        }
        let s = slots.remove(&quote.key).ok_or(Error::Conflict)?;
        Ok(Accepted {
            assessed: s.issue.assessed,
            price: s.price,
            bytes: s.issue.bytes,
            maximum: s.issue.maximum,
        })
    }
    /// Release only this exact unaccepted quote generation; accepted work is never cancelled by this operation.
    /// # Errors
    /// Foreign owners/peers, stale handles and caller failures cannot release another quote; an absent slot is already released.
    pub fn discard(&self, context: &Context<'_>, quote: &Quote, peer: Peer) -> Result<(), Error> {
        tracing::trace!("Releasing one exact unaccepted quote without changing accepted work");
        let domain = quote.domain.upgrade().ok_or(Error::Denied)?;
        if !Arc::ptr_eq(&domain, &self.domain) || peer.local != self.local || peer.key != quote.key
        {
            return Err(Error::Denied);
        }
        let action = if peer.key.requester == self.local {
            Action::Request
        } else {
            Action::Invoke
        };
        context.check(
            &self.authority,
            self.local,
            action,
            quote.bytes.bytes().len(),
        )?;
        let mut slots = self.slots.lock().map_err(|_| Error::Unavailable)?;
        let Some(slot) = slots.get(&quote.key) else {
            return Ok(());
        };
        if !Arc::ptr_eq(&slot.generation, &quote.generation) {
            return Err(Error::Conflict);
        }
        if slot.issue.assessed.authorized.peer.remote != peer.remote {
            return Err(Error::Denied);
        }
        slots.remove(&quote.key);
        Ok(())
    }
    /// Release unaccepted quotes on a trusted disconnect event; accepted work is coordinator-owned.
    /// # Errors
    /// Requires receiving-owner invocation authority; lock poisoning returns Unavailable.
    pub fn disconnect(&self, context: &Context<'_>, connection: [u8; 32]) -> Result<(), Error> {
        tracing::trace!("Releasing unaccepted quote pins after disconnect");
        let mut slots = self.slots.lock().map_err(|_| Error::Unavailable)?;
        if context
            .check(&self.authority, self.local, Action::Invoke, 0)
            .is_err()
        {
            context.check(&self.authority, self.local, Action::Request, 0)?;
        }
        slots.retain(|key, _| key.connection != connection);
        Ok(())
    }
}
