//! Local Identity lifecycle over explicit Cryptography material and protected-store contracts.
use super::{
    api::{IdentityId, IdentityProvider, PublicCredential, PublicIdentity, validate_contract},
    authority::{Authority, Capability, ReadPublic},
    config::Limits,
    error::IdentityError,
    ports::KeyStore,
    record::{CredentialRecord, RECORD_BYTES},
};
use crate::modules::cryptography::{identity_keys::IdentityKeys, ports::Entropy};
use std::sync::Arc;

/// Opaque live private authority tied to one provider domain; never a public or peer credential.
pub struct IdentityHandle {
    pub(super) keys: IdentityKeys,
    pub(super) public: PublicCredential,
    domain: Arc<()>,
}
/// Direct local lifecycle provider; the store decides durability and entropy supplies fresh independent keys.
pub struct Identity<S, E> {
    pub(super) store: S,
    entropy: E,
    pub(super) limits: Limits,
    pub(super) authority: Authority,
    domain: Arc<()>,
}
impl<S: KeyStore<Record = CredentialRecord>, E: Entropy> Identity<S, E> {
    /// Compose explicit dependencies and validate the local contract version before effects.
    /// # Errors
    /// Rejects unsupported local versions and a record budget unable to hold both keys.
    pub fn new(store: S, entropy: E, limits: Limits, version: u16) -> Result<Self, IdentityError> {
        tracing::trace!("Composing a protected local Identity provider");
        validate_contract(version)?;
        if limits.record_bytes() < RECORD_BYTES {
            return Err(IdentityError::ResourceLimit);
        }
        Ok(Self {
            store,
            entropy,
            limits,
            authority: Authority::new(),
            domain: Arc::new(()),
        })
    }
    /// Grant a local operation capability for an existing handle after administrative authorization.
    /// # Errors
    /// Rejects foreign handles, denied administration and invalid capability budgets.
    pub fn grant<P>(
        &self,
        admin: &S::Administration,
        handle: &IdentityHandle,
        maximum_bytes: usize,
    ) -> Result<Capability<P>, IdentityError> {
        tracing::trace!("Granting scoped authority for a loaded local identity");
        self.store.authorize(admin, 0)?;
        self.check_handle(handle)?;
        self.authority
            .grant(handle.public.identity.id(), maximum_bytes)
    }
    /// Read a public credential after validating scoped local read authority.
    /// # Errors
    /// Rejects foreign handles or denied/revoked capabilities; never returns private inputs.
    pub fn public(
        &self,
        handle: &IdentityHandle,
        capability: &Capability<ReadPublic>,
    ) -> Result<PublicCredential, IdentityError> {
        tracing::trace!("Reading an authorized local public credential");
        self.check_handle(handle)?;
        self.authority
            .check(capability, handle.public.identity.id(), 0)?;
        // Revalidate the retained private/public association before publishing its public snapshot.
        let credential = handle.keys.credential()?;
        if credential.identity_id != handle.public.identity.id().0
            || credential.encryption_public_key != handle.public.encryption_public_key
        {
            return Err(IdentityError::IdentityMismatch);
        }
        Ok(handle.public)
    }
    // Associate private authority only with this provider; a copied public identity never constructs a handle.
    pub(super) fn check_handle(&self, handle: &IdentityHandle) -> Result<(), IdentityError> {
        tracing::trace!("Checking local identity handle ownership");
        if !Arc::ptr_eq(&self.domain, &handle.domain) {
            return Err(IdentityError::AccessDenied);
        }
        Ok(())
    }
    // Build a complete public snapshot from private authority already validated by Cryptography.
    pub(super) fn handle(
        &self,
        keys: IdentityKeys,
    ) -> Result<(IdentityHandle, PublicCredential), IdentityError> {
        tracing::trace!("Publishing a complete local identity handle");
        let credential = keys.credential()?;
        let public = PublicCredential {
            identity: PublicIdentity {
                id: IdentityId(credential.identity_id),
                signing_public_key: credential.signing_public_key,
            },
            encryption_public_key: credential.encryption_public_key,
            binding_signature: credential.binding_signature,
        };
        Ok((
            IdentityHandle {
                keys,
                public,
                domain: self.domain.clone(),
            },
            public,
        ))
    }
}
impl<S: KeyStore<Record = CredentialRecord>, E: Entropy> IdentityProvider for Identity<S, E> {
    type Handle = IdentityHandle;
    type Administration = S::Administration;
    /// Load a complete existing credential after authorization, without consuming entropy.
    /// # Errors
    /// Preserves store failures and rejects malformed or inconsistent records without repair.
    fn load(
        &mut self,
        admin: &S::Administration,
    ) -> Result<(IdentityHandle, PublicCredential), IdentityError> {
        tracing::trace!("Loading and validating an existing protected identity");
        self.store.authorize(admin, RECORD_BYTES)?;
        let record = self.store.open(admin)?;
        self.handle(record.decode()?)
    }
    /// Create both independent pairs and report success only after complete durable installation.
    /// # Errors
    /// Checks authority before entropy; propagates failures and uncertain outcomes without retries or replacement.
    fn create(
        &mut self,
        admin: &S::Administration,
    ) -> Result<(IdentityHandle, PublicCredential), IdentityError> {
        tracing::trace!("Creating and durably installing a complete local identity");
        self.store.authorize(admin, RECORD_BYTES)?;
        let keys = IdentityKeys::generate(&mut self.entropy)?;
        let record = CredentialRecord::encode(&keys, self.limits)?;
        // Construct all fallible public output before committing the complete association.
        let result = self.handle(keys)?;
        self.store.install(admin, record)?;
        Ok(result)
    }
}
#[cfg(test)]
#[path = "tests/provider.rs"]
mod tests;
