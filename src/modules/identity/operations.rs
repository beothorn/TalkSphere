//! Authorized private operations over loaded handles; callers own protocol transcript construction.
use super::{
    authority::{Capability, Decrypt, Sign},
    error::IdentityError,
    ports::KeyStore,
    provider::{Identity, IdentityHandle},
    record::CredentialRecord,
};
use crate::modules::cryptography::{
    api::DecryptRequest,
    config::ResourceLimits,
    envelope::decrypt,
    ports::Entropy,
    primitives::{agreement, signature},
    secrets::{Secret, Shared},
};

/// Explicit local format choice; Identity never guesses an object's service semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeFormat {
    /// CR2SE version-one generic identity envelope.
    GenericV1,
    /// Service-specific constructions require their own unimplemented adapters.
    ServiceSpecific,
}

impl<S: KeyStore<Record = CredentialRecord>, E: Entropy> Identity<S, E> {
    /// Authenticate a complete generic envelope for the loaded local identity and return guarded plaintext.
    /// # Errors
    /// Rejects foreign/denied authority, unsupported formats, excessive sizes, malformed objects,
    /// wrong recipient fields and failed authentication without returning provisional plaintext.
    pub fn decrypt(
        &self,
        handle: &IdentityHandle,
        capability: &Capability<Decrypt>,
        format: EnvelopeFormat,
        object: &[u8],
    ) -> Result<zeroize::Zeroizing<Vec<u8>>, IdentityError> {
        tracing::trace!(
            "Decrypting an explicitly selected envelope through scoped Identity authority"
        );
        self.check_handle(handle)?;
        self.authority
            .check(capability, handle.public.identity.id(), object.len())?;
        if format != EnvelopeFormat::GenericV1 {
            return Err(IdentityError::Unsupported);
        }
        let limits =
            ResourceLimits::new(self.limits.envelope_bytes(), self.limits.envelope_bytes())?;
        Ok(zeroize::Zeroizing::new(decrypt::decrypt(DecryptRequest {
            object,
            identity_id: handle.public.identity.id().0,
            encryption_public_key: handle.public.encryption_public_key,
            private_key: handle.keys.encryption(),
            limits,
        })?))
    }
    /// Perform X25519 agreement between the loaded persistent encryption key and a remote public key, for a service-specific
    /// construction (such as the Messaging payload) whose key derivation and authentication stay with the caller.
    /// The private key never leaves Identity; only the protected shared secret is returned.
    /// # Errors
    /// Denies foreign handles or invalid `Decrypt` authority and rejects a low-order remote key.
    pub fn agree(
        &self,
        handle: &IdentityHandle,
        capability: &Capability<Decrypt>,
        remote_public_key: &[u8; 32],
    ) -> Result<Secret<Shared>, IdentityError> {
        tracing::trace!("Agreeing a shared secret through scoped Identity authority");
        self.check_handle(handle)?;
        self.authority.check(
            capability,
            handle.public.identity.id(),
            remote_public_key.len(),
        )?;
        Ok(agreement::persistent(
            handle.keys.encryption(),
            remote_public_key,
        )?)
    }
    /// Sign the caller's exact protocol bytes using scoped local signing authority.
    /// The caller supplies domain separation, role, freshness and replay policy.
    /// # Errors
    /// Denies foreign handles or invalid authority and rejects either capability or configured byte-limit excess.
    pub fn sign(
        &self,
        handle: &IdentityHandle,
        capability: &Capability<Sign>,
        message: &[u8],
    ) -> Result<[u8; 64], IdentityError> {
        tracing::trace!("Signing exact protocol bytes through scoped Identity authority");
        self.check_handle(handle)?;
        self.authority
            .check(capability, handle.public.identity.id(), message.len())?;
        Ok(signature::sign(
            handle.keys.signing(),
            message,
            self.limits.signing_bytes(),
        )?)
    }
}
