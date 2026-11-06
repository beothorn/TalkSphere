//! Trusted local Board authority; tokens are process-local capabilities, never peer metadata.
use super::Error;
use crate::modules::identity::api::IdentityId;
use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, Ordering},
};

/// Allowed local action; grants are identity-bound and byte-budgeted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Replace the complete local publication.
    Publish,
    /// Read public snapshots or definitions.
    Read,
    /// Install attributed remote cache data through a trusted adapter.
    Cache,
    /// Select terms without invoking or spending.
    Select,
}
/// Opaque grant, valid only while its issuing authority remains live and unrevoked.
pub struct Grant {
    domain: Weak<AtomicBool>,
    publisher: IdentityId,
    action: Action,
    bytes: usize,
}
/// Issuer held by trusted composition; possession allows issuance of Board grants.
pub struct Authority(Arc<AtomicBool>);
impl Default for Authority {
    /// Construct a fresh live domain with the same behavior as new.
    fn default() -> Self {
        tracing::trace!("Creating default Board authority");
        Self::new()
    }
}
impl Authority {
    /// Create an independent live authority domain with no grants.
    pub fn new() -> Self {
        tracing::trace!("Creating a trusted Board authority domain");
        Self(Arc::new(AtomicBool::new(true)))
    }
    /// Issue a grant after composition has applied its local authorization policy.
    /// # Errors
    /// Denies revoked issuers and rejects zero byte budgets.
    pub fn grant(
        &self,
        publisher: IdentityId,
        action: Action,
        bytes: usize,
    ) -> Result<Grant, Error> {
        tracing::trace!("Issuing a bounded local Board grant");
        if !self.0.load(Ordering::SeqCst) {
            return Err(Error::Denied);
        }
        if bytes == 0 {
            return Err(Error::Limit);
        }
        Ok(Grant {
            domain: Arc::downgrade(&self.0),
            publisher,
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
        publisher: IdentityId,
        action: Action,
        bytes: usize,
    ) -> Result<(), Error> {
        tracing::trace!("Checking Board authority before an operation");
        let domain = grant.domain.upgrade().ok_or(Error::Denied)?;
        if !Arc::ptr_eq(&self.0, &domain)
            || !domain.load(Ordering::SeqCst)
            || grant.publisher != publisher
            || grant.action != action
        {
            return Err(Error::Denied);
        }
        if bytes > grant.bytes {
            return Err(Error::Limit);
        }
        Ok(())
    }
    /// Permanently invalidate this domain's grants without changing durable publication.
    pub fn revoke(&self) {
        tracing::trace!("Revoking Board operation grants");
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
        assert!(matches!(a.grant(id, Action::Publish, 0), Err(Error::Limit)));
        let g = a.grant(id, Action::Publish, 8).unwrap();
        assert_eq!(a.check(&g, id, Action::Publish, 8), Ok(()));
        assert_eq!(b.check(&g, id, Action::Publish, 1), Err(Error::Denied));
        assert_eq!(
            a.check(&g, IdentityId([2; 32]), Action::Publish, 1),
            Err(Error::Denied)
        );
        assert_eq!(a.check(&g, id, Action::Read, 1), Err(Error::Denied));
        assert_eq!(a.check(&g, id, Action::Publish, 9), Err(Error::Limit));
        a.revoke();
        assert_eq!(a.check(&g, id, Action::Publish, 1), Err(Error::Denied));
        assert!(matches!(a.grant(id, Action::Read, 1), Err(Error::Denied)));
    }
}
