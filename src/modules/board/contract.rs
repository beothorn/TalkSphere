//! Direct-call boundary shared by local composition, future Services and Peer Control adapters.
use super::{
    Error,
    authority::{Action, Authority, Grant},
    config::Limits,
};
use crate::modules::identity::api::IdentityId;
use std::{sync::Arc, time::Instant};

/// Opaque immutable local snapshot; never serialized as a peer revision field.
/// Construction is reserved for the Board owner after complete validation.
#[derive(Clone)]
pub struct Snapshot {
    pub(super) publisher: IdentityId,
    pub(super) board: Arc<[u8]>,
    pub(super) data: Option<Arc<super::prepared::Prepared>>,
    pub(super) domain: std::sync::Weak<()>,
}
impl Snapshot {
    /// Return the bound publisher without asserting forwarded-document authentication.
    pub fn publisher(&self) -> IdentityId {
        tracing::trace!("Reading snapshot publisher association");
        self.publisher
    }
    /// Borrow exact immutable Board JSON for this pinned revision's lifetime.
    pub fn bytes(&self) -> &[u8] {
        tracing::trace!("Reading pinned Board document bytes");
        &self.board
    }
}
/// Per-call local control; peer JSON cannot grant or extend this authority.
pub struct Context<'a> {
    /// Grant issued by trusted composition for this operation.
    pub grant: &'a Grant,
    /// Monotonic deadline checked before effects.
    pub deadline: Instant,
    /// Caller cancellation observed before effects; concurrent cancellation is adapter-owned.
    pub cancelled: bool,
}
impl Context<'_> {
    /// Check cancellation/deadline and local grant against the trusted receiving owner.
    /// # Errors
    /// Returns Cancelled before effects, or Denied/Limit for invalid authority/budget.
    pub fn check(
        &self,
        authority: &Authority,
        publisher: IdentityId,
        action: Action,
        bytes: usize,
    ) -> Result<(), Error> {
        tracing::trace!("Validating Board call context before effects");
        if self.cancelled || Instant::now() >= self.deadline {
            return Err(Error::Cancelled);
        }
        authority.check(self.grant, publisher, action, bytes)
    }
}
/// Initial module boundary; unsupported operations fail explicitly until later implementation steps.
/// No networking, persistent writes or service registration occur at construction.
pub struct Boundary {
    authority: Authority,
    limits: Limits,
}
impl Boundary {
    /// Take ownership of trusted authority and validated finite resource policy.
    /// # Errors
    /// Returns Invalid when resource configuration is inconsistent.
    pub fn new(authority: Authority, limits: Limits) -> Result<Self, Error> {
        tracing::trace!("Constructing the Board boundary without platform effects");
        limits.validate()?;
        Ok(Self { authority, limits })
    }
    /// Read one immutable local snapshot under explicit read authority.
    /// # Errors
    /// Returns call-control errors first, then Unsupported until snapshot ownership is implemented.
    pub fn read(&self, context: &Context<'_>, publisher: IdentityId) -> Result<Snapshot, Error> {
        tracing::trace!("Requesting a local Board snapshot");
        context.check(&self.authority, publisher, Action::Read, 0)?;
        Err(Error::Unsupported)
    }
    /// Return validated policy for bounded adapter construction.
    pub fn limits(&self) -> Limits {
        tracing::trace!("Reading Board resource policy");
        self.limits
    }
}
/// Logical contract version understood by this direct binding, separate from Board JSON version.
pub const CONTRACT_VERSION: u32 = 1;

/// Required logical ports; implementations own their validated candidate and selection types.
/// This trait specifies the boundary without exposing private storage or requiring a runtime.
pub trait BoardPorts {
    /// Immutable validated candidate produced only after complete candidate checks.
    type Candidate;
    /// Immutable validated definition bytes/metadata tied to one snapshot.
    type Definition;
    /// Trusted retrieval association supplied by the local peer adapter, not JSON.
    type Retrieval;
    /// Caller-selected contract requirements and explicit supported semantics.
    type SelectionRequest;
    /// Immutable terms and usability result; this never authorizes an invocation.
    type Selection;

    /// Commit a complete prepared publication under Publish authority.
    /// # Errors
    /// Rejects authority/control/resource failures; distinguishes precommit Storage from Unknown.
    fn publish(
        &mut self,
        context: &Context<'_>,
        candidate: Self::Candidate,
    ) -> Result<Snapshot, Error>;

    /// Read a matching definition from an explicitly pinned snapshot under Read authority.
    /// # Errors
    /// Rejects foreign/expired scope and missing IDs without version substitution.
    fn definition(
        &self,
        context: &Context<'_>,
        snapshot: &Snapshot,
        id: &str,
    ) -> Result<Self::Definition, Error>;

    /// Install a validated remote result through a trusted Cache-authorized retrieval association.
    /// # Errors
    /// Rejects invalid data, forged association, cancellation and capacity exhaustion.
    fn cache(
        &mut self,
        context: &Context<'_>,
        retrieval: Self::Retrieval,
    ) -> Result<Snapshot, Error>;

    /// Select immutable terms under Select authority without execution or payment.
    /// # Errors
    /// Rejects missing/stale selections, unsupported semantics and scope/control failures.
    fn select(
        &self,
        context: &Context<'_>,
        snapshot: &Snapshot,
        request: Self::SelectionRequest,
    ) -> Result<Self::Selection, Error>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    // Valid caller authority still receives explicit unavailable status; cancellation wins first.
    #[test]
    fn scaffold_never_fabricates_a_snapshot() {
        let authority = Authority::default();
        let id = IdentityId([1; 32]);
        let grant = authority.grant(id, Action::Read, 1).unwrap();
        let boundary = Boundary::new(authority, Limits::default()).unwrap();
        let mut context = Context {
            grant: &grant,
            deadline: Instant::now() + Duration::from_secs(10),
            cancelled: false,
        };
        assert!(matches!(
            boundary.read(&context, id),
            Err(Error::Unsupported)
        ));
        assert_eq!(boundary.limits(), Limits::default());
        context.cancelled = true;
        assert!(matches!(boundary.read(&context, id), Err(Error::Cancelled)));
        context.cancelled = false;
        context.deadline = Instant::now();
        assert!(matches!(boundary.read(&context, id), Err(Error::Cancelled)));
        assert!(matches!(
            Boundary::new(
                Authority::new(),
                Limits {
                    depth: 0,
                    ..Limits::default()
                }
            ),
            Err(Error::Invalid)
        ));
    }
    // Owner-created pins expose exact immutable bytes and identity; clones preserve the same content.
    #[test]
    fn pinned_contract_preserves_bytes() {
        let s = Snapshot {
            publisher: IdentityId([3; 32]),
            data: None,
            domain: std::sync::Weak::new(),
            board: Arc::from(&b"{}"[..]),
        };
        let other = s.clone();
        assert_eq!(other.publisher(), IdentityId([3; 32]));
        assert_eq!(other.bytes(), b"{}");
    }
}
