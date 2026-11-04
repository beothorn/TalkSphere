//! Scoped real Identity dependency for Transport; network transcripts remain owned by Transport.
use crate::modules::{
    cryptography::{credential, ports::Entropy},
    identity::{
        authority::{Capability, ReadPublic, Sign},
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

/// Owned provider and loaded authority; retaining this adapter also retains the protected store lock.
pub struct TransportIdentity<S, E> {
    provider: Identity<S, E>,
    handle: IdentityHandle,
    read: Capability<ReadPublic>,
    sign: Capability<Sign>,
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
            provider,
            handle,
            read,
            sign,
        })
    }
}
impl<S: KeyStore<Record = CredentialRecord> + Send, E: Entropy> IdentityPort
    for TransportIdentity<S, E>
{
    /// Project validated public signing credentials after scoped read authorization.
    fn public_identity(&self) -> Result<PublicIdentity, DependencyError> {
        tracing::trace!("Reading real Transport identity credentials");
        let public = self
            .provider
            .public(&self.handle, &self.read)
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
        self.provider
            .sign(&self.handle, &self.sign, message.as_bytes())
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
