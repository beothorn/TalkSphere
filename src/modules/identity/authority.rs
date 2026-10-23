//! Purpose-scoped local capabilities; trusted composition alone holds the issuing authority.
use super::{api::IdentityId, error::IdentityError};
use std::{
    marker::PhantomData,
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, Ordering},
    },
};

/// Local administration purpose, distinct from every ordinary operation.
pub enum Administration {}
/// Read public credentials for a selected local identity.
pub enum ReadPublic {}
/// Sign the exact bytes supplied by an authorized protocol adapter.
pub enum Sign {}
/// Decrypt a selected format for the bound local identity.
pub enum Decrypt {}
/// Privileged protected backup purpose.
pub enum Backup {}
/// Privileged protected import purpose.
pub enum Import {}

/// Opaque identity- and purpose-bound authority with no wire representation.
/// ```compile_fail
/// use talksphere::modules::identity::authority::{Capability,Sign,Decrypt};
/// fn confuse(key: Capability<Sign>) -> Capability<Decrypt> { key }
/// ```
pub struct Capability<P> {
    owner: Weak<AtomicBool>,
    identity: Option<IdentityId>,
    maximum_bytes: usize,
    purpose: PhantomData<P>,
}
/// Issuer held only by trusted local composition; never constructed from peer metadata.
/// Revoking or dropping it invalidates all outstanding capabilities.
pub struct Authority(Arc<AtomicBool>);
impl Default for Authority {
    /// Construct a fresh local authority domain with no network-derived identity.
    fn default() -> Self {
        tracing::trace!("Creating the default local authority domain");
        Self::new()
    }
}
impl Authority {
    /// Create a fresh issuer; handing this value to a caller grants administrative power.
    pub fn new() -> Self {
        tracing::trace!("Creating a trusted local identity authority domain");
        Self(Arc::new(AtomicBool::new(true)))
    }
    /// Issue operation authority for one identity and a finite byte budget.
    /// # Errors
    /// Rejects revoked issuers and zero budgets; callers must first apply local policy.
    pub fn grant<P>(
        &self,
        identity: IdentityId,
        maximum_bytes: usize,
    ) -> Result<Capability<P>, IdentityError> {
        tracing::trace!("Granting identity-scoped local operation authority");
        self.issue(Some(identity), maximum_bytes)
    }
    /// Issue local administration for creation/loading before an identity is known.
    /// # Errors
    /// Rejects revoked issuers and zero budgets; this is not a peer-facing operation.
    pub fn administration(
        &self,
        maximum_bytes: usize,
    ) -> Result<Capability<Administration>, IdentityError> {
        tracing::trace!("Granting protected local administrative authority");
        self.issue(None, maximum_bytes)
    }
    /// Revoke every capability in this domain; existing durable state is unchanged.
    pub fn revoke(&self) {
        tracing::trace!("Revoking a local identity authority domain");
        self.0.store(false, Ordering::SeqCst);
    }
    /// Validate issuer, lifetime, bound identity and byte budget before any effect.
    /// The consuming operation fixes P in its signature rather than accepting an arbitrary purpose.
    /// # Errors
    /// Denies foreign/revoked/wrong-identity capabilities; excessive bytes return ResourceLimit.
    pub fn check<P>(
        &self,
        capability: &Capability<P>,
        identity: IdentityId,
        bytes: usize,
    ) -> Result<(), IdentityError> {
        tracing::trace!("Checking scoped identity operation authority");
        self.check_scope(capability, Some(identity), bytes)
    }
    /// Check unbound administration before inspecting protected storage or creating an identity.
    /// # Errors
    /// Denies foreign, revoked or identity-bound administration and enforces its byte budget.
    pub fn check_administration(
        &self,
        capability: &Capability<Administration>,
        bytes: usize,
    ) -> Result<(), IdentityError> {
        tracing::trace!("Checking protected administrative authority");
        self.check_scope(capability, None, bytes)
    }
    // Centralize issue invariants without exposing capability fields to local callers.
    fn issue<P>(
        &self,
        identity: Option<IdentityId>,
        maximum_bytes: usize,
    ) -> Result<Capability<P>, IdentityError> {
        tracing::trace!("Issuing an opaque local capability");
        if !self.0.load(Ordering::SeqCst) {
            return Err(IdentityError::AccessDenied);
        }
        if maximum_bytes == 0 {
            return Err(IdentityError::InvalidInput);
        }
        Ok(Capability {
            owner: Arc::downgrade(&self.0),
            identity,
            maximum_bytes,
            purpose: PhantomData,
        })
    }
    // Authority failure takes precedence over resource/existence information.
    fn check_scope<P>(
        &self,
        capability: &Capability<P>,
        identity: Option<IdentityId>,
        bytes: usize,
    ) -> Result<(), IdentityError> {
        tracing::trace!("Checking local capability ownership and lifetime");
        let owner = capability
            .owner
            .upgrade()
            .ok_or(IdentityError::AccessDenied)?;
        if !Arc::ptr_eq(&owner, &self.0)
            || !owner.load(Ordering::SeqCst)
            || capability.identity != identity
        {
            return Err(IdentityError::AccessDenied);
        }
        if bytes > capability.maximum_bytes {
            return Err(IdentityError::ResourceLimit);
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    // Foreign, expired, revoked and wrongly bound handles fail before resource-sensitive operations.
    #[test]
    fn issuer_identity_lifetime_purpose_and_budget_are_explicit() {
        let issuer = Authority::new();
        let other = Authority::default();
        let id = IdentityId([1; 32]);
        assert!(issuer.grant::<Sign>(id, 0).is_err());
        let sign = issuer.grant::<Sign>(id, 3).unwrap();
        issuer.check(&sign, id, 3).unwrap();
        assert_eq!(
            issuer.check(&sign, id, 4),
            Err(IdentityError::ResourceLimit)
        );
        assert_eq!(other.check(&sign, id, 0), Err(IdentityError::AccessDenied));
        assert_eq!(
            issuer.check(&sign, IdentityId([2; 32]), 0),
            Err(IdentityError::AccessDenied)
        );
        let admin = issuer.administration(4).unwrap();
        issuer.check_administration(&admin, 4).unwrap();
        assert_eq!(
            issuer.check(&admin, id, 0),
            Err(IdentityError::AccessDenied)
        );
        let scoped_admin = issuer.grant::<Administration>(id, 4).unwrap();
        assert_eq!(
            issuer.check_administration(&scoped_admin, 0),
            Err(IdentityError::AccessDenied)
        );
        issuer.revoke();
        assert_eq!(issuer.check(&sign, id, 0), Err(IdentityError::AccessDenied));
        assert!(issuer.grant::<Sign>(id, 1).is_err());
        drop(issuer);
        assert_eq!(other.check(&sign, id, 0), Err(IdentityError::AccessDenied));
    }
}
