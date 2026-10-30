//! Authorized private operations over loaded handles; callers own protocol transcript construction.
use super::{
    authority::{Capability, Sign},
    error::IdentityError,
    ports::KeyStore,
    provider::{Identity, IdentityHandle},
    record::CredentialRecord,
};
use crate::modules::cryptography::{ports::Entropy, primitives::signature};

impl<S: KeyStore<Record = CredentialRecord>, E: Entropy> Identity<S, E> {
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
