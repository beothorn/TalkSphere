//! Direct-call boundary shared by local composition, future Services and Peer Control adapters.
use super::{
    Error,
    authority::{Action, Authority, Grant},
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
/// Logical contract version understood by this direct binding, separate from Board JSON version.
pub const CONTRACT_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    // Deadline/cancellation checks precede local operation authority and produce no effects.
    #[test]
    fn call_context_checks_control_before_authority() {
        let authority = Authority::new();
        let id = IdentityId([1; 32]);
        let grant = authority.grant(id, Action::Read, 1).unwrap();
        let mut context = Context {
            grant: &grant,
            deadline: Instant::now() + Duration::from_secs(10),
            cancelled: false,
        };
        assert_eq!(context.check(&authority, id, Action::Read, 1), Ok(()));
        context.cancelled = true;
        assert_eq!(
            context.check(&authority, id, Action::Read, 1),
            Err(Error::Cancelled)
        );
        context.cancelled = false;
        context.deadline = Instant::now();
        assert_eq!(
            context.check(&authority, id, Action::Read, 1),
            Err(Error::Cancelled)
        );
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
