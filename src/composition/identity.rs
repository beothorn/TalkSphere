//! Scoped real Identity dependency for Transport; network transcripts remain owned by Transport.
use crate::modules::{
    cryptography::{
        credential,
        ports::Entropy,
        secrets::{Secret, Shared},
    },
    identity::{
        api::PublicCredential,
        authority::{Capability, Decrypt, ReadPublic, Sign},
        error::IdentityError,
        ports::KeyStore,
        provider::{Identity, IdentityHandle},
        record::CredentialRecord,
    },
    transport::ports::{
        DependencyError,
        identity::{AuthenticationMessage, Identity as IdentityPort, IdentityId, PublicIdentity},
    },
};
use std::sync::Arc;

// The protected provider, its loaded handle and every scoped grant; shared so one process-wide credential serves Transport and
// the service modules without a second store lock.
struct Inner<S, E> {
    provider: Identity<S, E>,
    handle: IdentityHandle,
    read: Capability<ReadPublic>,
    sign: Capability<Sign>,
    decrypt: Option<Capability<Decrypt>>,
}

/// Owned provider and loaded authority; retaining this adapter (or any [`ProtectedIdentity`] cloned from it) also retains the
/// protected store lock.
pub struct TransportIdentity<S, E> {
    inner: Arc<Inner<S, E>>,
}

/// Shared signing and agreement authority over the same protected identity Transport authenticates with, for service modules
/// such as Messaging. It exposes only signing of caller-built bytes and X25519 agreement; private keys never leave Identity.
pub struct ProtectedIdentity<S, E> {
    inner: Arc<Inner<S, E>>,
}
impl<S, E> Clone for ProtectedIdentity<S, E> {
    // Share the same protected credential.
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}
impl<S: KeyStore<Record = CredentialRecord>, E: Entropy> TransportIdentity<S, E> {
    /// Compose loaded authority with purpose-specific grants and validate public-read access immediately.
    /// # Errors
    /// Rejects foreign handles and denied read authority; signing authority is checked on each AUTH request.
    pub fn new(
        provider: Identity<S, E>,
        handle: IdentityHandle,
        read: Capability<ReadPublic>,
        sign: Capability<Sign>,
    ) -> Result<Self, IdentityError> {
        tracing::trace!("Composing real scoped Identity authority for Transport");
        provider.public(&handle, &read)?;
        Ok(Self {
            inner: Arc::new(Inner {
                provider,
                handle,
                read,
                sign,
                decrypt: None,
            }),
        })
    }
    /// Add service-specific decryption authority before the identity is shared.
    /// # Errors
    /// `Unavailable` when the identity is already shared and can no longer be changed.
    pub fn with_decrypt(mut self, decrypt: Capability<Decrypt>) -> Result<Self, IdentityError> {
        tracing::trace!("Adding scoped decryption authority to the shared Identity");
        Arc::get_mut(&mut self.inner)
            .ok_or(IdentityError::Unavailable)?
            .decrypt = Some(decrypt);
        Ok(self)
    }
    /// A shared handle for service modules; the Transport adapter keeps working alongside it.
    pub fn protected(&self) -> ProtectedIdentity<S, E> {
        tracing::trace!("Sharing the protected identity with a service module");
        ProtectedIdentity {
            inner: self.inner.clone(),
        }
    }
}
impl<S: KeyStore<Record = CredentialRecord>, E: Entropy> ProtectedIdentity<S, E> {
    /// The local identity and its public credential.
    /// # Errors
    /// Denied public-read authority.
    pub fn public(&self) -> Result<PublicCredential, IdentityError> {
        tracing::trace!("Reading the shared identity's public credential");
        self.inner
            .provider
            .public(&self.inner.handle, &self.inner.read)
    }
    /// Sign exactly `message` with the identity's signing key.
    /// # Errors
    /// Identity's authority and size failures.
    pub fn sign(&self, message: &[u8]) -> Result<[u8; 64], IdentityError> {
        tracing::trace!("Signing through the shared protected identity");
        self.inner
            .provider
            .sign(&self.inner.handle, &self.inner.sign, message)
    }
    /// X25519 agreement with a remote public key under the identity's decrypt authority.
    /// # Errors
    /// `AccessDenied` when no decrypt authority was configured, and Identity's failures otherwise.
    pub fn agree(&self, remote: &[u8; 32]) -> Result<Secret<Shared>, IdentityError> {
        tracing::trace!("Agreeing through the shared protected identity");
        let capability = self
            .inner
            .decrypt
            .as_ref()
            .ok_or(IdentityError::AccessDenied)?;
        self.inner
            .provider
            .agree(&self.inner.handle, capability, remote)
    }
}
impl<S: KeyStore<Record = CredentialRecord> + Send + Sync, E: Entropy + Send + Sync> IdentityPort
    for TransportIdentity<S, E>
{
    /// Project validated public signing credentials after scoped read authorization.
    fn public_identity(&self) -> Result<PublicIdentity, DependencyError> {
        tracing::trace!("Reading real Transport identity credentials");
        let public = self
            .inner
            .provider
            .public(&self.inner.handle, &self.inner.read)
            .map_err(map_error)?;
        Ok(PublicIdentity {
            id: IdentityId(public.identity.id().0),
            signing_public_key: public.identity.signing_public_key(),
        })
    }
    /// Derive the canonical public ID; this operation grants no authentication.
    fn identify(&self, key: &[u8; 32]) -> Result<IdentityId, DependencyError> {
        tracing::trace!("Deriving a Transport peer identity identifier");
        credential::identity_id(key)
            .map(IdentityId)
            .map_err(|error| map_error(error.into()))
    }
    /// Sign only a Transport-constructed AUTH message through scoped signing authority.
    fn sign_authentication(
        &mut self,
        message: &AuthenticationMessage,
    ) -> Result<[u8; 64], DependencyError> {
        tracing::trace!("Signing Transport authentication through real Identity");
        self.inner
            .provider
            .sign(&self.inner.handle, &self.inner.sign, message.as_bytes())
            .map_err(map_error)
    }
}
// Preserve the distinctions representable by Transport's dependency contract without exposing private diagnostics.
fn map_error(error: IdentityError) -> DependencyError {
    tracing::trace!("Mapping an Identity dependency failure for Transport");
    match error {
        IdentityError::AccessDenied => DependencyError::Denied,
        IdentityError::IdentityMismatch | IdentityError::InvalidProof => {
            DependencyError::VerificationFailed
        }
        IdentityError::InvalidInput | IdentityError::Unsupported | IdentityError::ResourceLimit => {
            DependencyError::InvalidInput
        }
        IdentityError::Unavailable | IdentityError::Conflict | IdentityError::OutcomeUnknown => {
            DependencyError::Unavailable
        }
    }
}
#[cfg(all(test, target_os = "linux"))]
#[path = "identity/tests.rs"]
mod tests;
