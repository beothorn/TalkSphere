//! Trusted local Services authority; tokens are process-local capabilities, never peer metadata.
use super::Error;
use crate::modules::identity::api::IdentityId;
use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, Ordering},
};

/// Allowed local action; grants are identity-bound and byte-budgeted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Register or drain local contract implementations.
    Register,
    /// Observe authorized invocation state.
    Observe,
    /// Reserve and coordinate a local accepted operation.
    Coordinate,
    /// Submit authenticated provider-side invocations.
    Invoke,
    /// Authorize local requester input and bounded spending.
    Request,
    /// Recover durable coordinator state and decision delivery.
    Recover,
}
/// Opaque grant, valid only while its issuing authority remains live and unrevoked.
pub struct Grant {
    domain: Weak<AtomicBool>,
    local_identity: IdentityId,
    action: Action,
    bytes: usize,
}
/// Issuer held by trusted composition; possession allows issuance of Services grants.
pub struct Authority(Arc<AtomicBool>);
impl Default for Authority {
    /// Construct a fresh live domain with the same behavior as new.
    fn default() -> Self {
        tracing::trace!("Creating default Services authority");
        Self::new()
    }
}
impl Authority {
    /// Create an independent live authority domain with no grants.
    pub fn new() -> Self {
        tracing::trace!("Creating a trusted Services authority domain");
        Self(Arc::new(AtomicBool::new(true)))
    }
    /// Issue a grant after composition has applied its local authorization policy.
    /// # Errors
    /// Denies revoked issuers and rejects zero byte budgets.
    pub fn grant(
        &self,
        local_identity: IdentityId,
        action: Action,
        bytes: usize,
    ) -> Result<Grant, Error> {
        tracing::trace!("Issuing a bounded local Services grant");
        if !self.0.load(Ordering::SeqCst) {
            return Err(Error::Denied);
        }
        if bytes == 0 {
            return Err(Error::Limit);
        }
        Ok(Grant {
            domain: Arc::downgrade(&self.0),
            local_identity,
            action,
            bytes,
        })
    }
    /// Check the exact issuer, identity, purpose and byte budget before effects.
    /// # Errors
    /// Returns Denied for foreign/revoked/wrong-purpose grants and Limit for excess bytes.
    pub fn check(
        &self,
        grant: &Grant,
        local_identity: IdentityId,
        action: Action,
        bytes: usize,
    ) -> Result<(), Error> {
        tracing::trace!("Checking Services authority before an operation");
        let domain = grant.domain.upgrade().ok_or(Error::Denied)?;
        if !Arc::ptr_eq(&self.0, &domain)
            || !domain.load(Ordering::SeqCst)
            || grant.local_identity != local_identity
            || grant.action != action
        {
            return Err(Error::Denied);
        }
        if bytes > grant.bytes {
            return Err(Error::Limit);
        }
        Ok(())
    }
    /// Permanently invalidate this domain's grants without changing durable obligations.
    pub fn revoke(&self) {
        tracing::trace!("Revoking Services operation grants");
        self.0.store(false, Ordering::SeqCst);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    // Grants cannot cross owners, identities, purposes or byte budgets and revocation is permanent.
    #[test]
    fn grants_are_scoped_and_revocable() {
        let a = Authority::new();
        let b = Authority::new();
        let id = IdentityId([1; 32]);
        assert!(matches!(
            a.grant(id, Action::Register, 0),
            Err(Error::Limit)
        ));
        let g = a.grant(id, Action::Register, 8).unwrap();
        assert_eq!(a.check(&g, id, Action::Register, 8), Ok(()));
        assert_eq!(b.check(&g, id, Action::Register, 1), Err(Error::Denied));
        assert_eq!(
            a.check(&g, IdentityId([2; 32]), Action::Register, 1),
            Err(Error::Denied)
        );
        assert_eq!(a.check(&g, id, Action::Observe, 1), Err(Error::Denied));
        assert_eq!(a.check(&g, id, Action::Register, 9), Err(Error::Limit));
        a.revoke();
        assert_eq!(a.check(&g, id, Action::Register, 1), Err(Error::Denied));
        assert!(matches!(
            a.grant(id, Action::Observe, 1),
            Err(Error::Denied)
        ));
    }
    // A dropped issuer ends grant authority; immutable public bytes already returned remain owned data.
    #[test]
    fn dropped_issuer_cannot_authorize() {
        let id = IdentityId([1; 32]);
        let grant = {
            let a = Authority::new();
            a.grant(id, Action::Observe, 8).unwrap()
        };
        assert_eq!(
            Authority::new().check(&grant, id, Action::Observe, 1),
            Err(Error::Denied)
        );
    }
}
