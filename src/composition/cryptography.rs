//! Real Cryptography adapter for Transport, leaving transcripts and counters in Transport.

use crate::modules::{
    cryptography::{
        api::{Hashing, KeyAgreement, Signatures},
        error::CryptoError,
        ports::Entropy,
        primitives::{integrity, kdf},
        provider::Provider,
        secrets::{Ephemeral, Integrity, Prk, Secret, Shared},
    },
    transport::ports::{
        DependencyError,
        crypto::{Cryptography, EphemeralKey},
    },
};

/// Owned real provider satisfying Transport's primitive port, without Identity substitution.
pub struct TransportCrypto<E> {
    provider: Provider<E>,
}
impl<E: Entropy> TransportCrypto<E> {
    /// Transfer a validated provider into the Transport adapter without I/O.
    pub fn new(provider: Provider<E>) -> Self {
        tracing::trace!("Composing the real Transport crypto dependency");
        Self { provider }
    }
}
impl<E: Entropy> Cryptography for TransportCrypto<E> {
    type EphemeralSecret = Secret<Ephemeral>;
    type SharedSecret = Secret<Shared>;
    type PseudorandomKey = Secret<Prk>;
    type IntegrityKey = Secret<Integrity>;
    /// Generate a fresh public nonce; map entropy failure without fallback.
    fn random_nonce(&mut self) -> Result<[u8; 32], DependencyError> {
        tracing::trace!("Generating a Transport nonce through real cryptography");
        self.provider.hello_nonce().map_err(map_error)
    }
    /// Generate consumed-purpose ephemeral authority or propagate entropy failure.
    fn ephemeral_key(&mut self) -> Result<EphemeralKey<Self::EphemeralSecret>, DependencyError> {
        tracing::trace!("Generating Transport ephemeral authority");
        let key = self.provider.generate_ephemeral_key().map_err(map_error)?;
        Ok(EphemeralKey {
            public_key: key.public_key,
            private_key: key.private_key,
        })
    }
    /// Hash exact transcript bytes under the provider ceiling.
    fn sha256(&self, bytes: &[u8]) -> Result<[u8; 32], DependencyError> {
        tracing::trace!("Hashing Transport transcript material");
        self.provider.sha256(bytes).map_err(map_error)
    }
    /// Verify a peer AUTH with actual Ed25519 and documented strict rejection policy.
    fn verify_signature(
        &self,
        public_key: &[u8; 32],
        message: &[u8],
        signature: &[u8; 64],
    ) -> Result<(), DependencyError> {
        tracing::trace!("Verifying actual Transport identity proof");
        self.provider
            .verify(public_key, message, signature)
            .map_err(map_error)
    }
    /// Consume private authority and reject all-zero agreement results.
    fn agree(
        &mut self,
        private_key: Self::EphemeralSecret,
        remote_key: &[u8; 32],
    ) -> Result<Self::SharedSecret, DependencyError> {
        tracing::trace!("Agreeing Transport shared material after proof verification");
        self.provider
            .agree_ephemeral(private_key, remote_key)
            .map_err(map_error)
    }
    /// Extract exact transcript-salted PRK, consuming shared material.
    fn extract(
        &mut self,
        salt: &[u8; 32],
        shared: Self::SharedSecret,
    ) -> Result<Self::PseudorandomKey, DependencyError> {
        tracing::trace!("Extracting the Transport PRK");
        kdf::extract(salt, shared, self.provider.maximum_bytes()).map_err(map_error)
    }
    /// Expand an exact directional label into a protected integrity key.
    fn expand_key(
        &mut self,
        key: &Self::PseudorandomKey,
        info: &[u8],
    ) -> Result<Self::IntegrityKey, DependencyError> {
        tracing::trace!("Expanding a directional Transport integrity key");
        kdf::expand_key(key, info, self.provider.maximum_bytes()).map_err(map_error)
    }
    /// Expand an exact directional label into a public nonce prefix.
    fn expand_nonce_prefix(
        &self,
        key: &Self::PseudorandomKey,
        info: &[u8],
    ) -> Result<[u8; 16], DependencyError> {
        tracing::trace!("Expanding a directional Transport nonce prefix");
        kdf::expand_nonce_prefix(key, info, self.provider.maximum_bytes()).map_err(map_error)
    }
    /// Compute the empty-plaintext tag over visible frame bytes, preserving caller nonce sequencing.
    fn tag(
        &self,
        key: &Self::IntegrityKey,
        nonce: &[u8; 24],
        authenticated_data: &[u8],
    ) -> Result<[u8; 16], DependencyError> {
        tracing::trace!("Creating a real Transport frame tag");
        integrity::tag(
            key,
            nonce,
            authenticated_data,
            self.provider.maximum_bytes(),
        )
        .map_err(map_error)
    }
    /// Verify the actual tag before Transport dispatch, returning a structured rejection.
    fn verify_tag(
        &self,
        key: &Self::IntegrityKey,
        nonce: &[u8; 24],
        authenticated_data: &[u8],
        tag: &[u8; 16],
    ) -> Result<(), DependencyError> {
        tracing::trace!("Checking a real Transport frame tag");
        integrity::verify_tag(
            key,
            nonce,
            authenticated_data,
            tag,
            self.provider.maximum_bytes(),
        )
        .map_err(map_error)
    }
}

// Preserve port failure categories without exposing underlying input or private material.
fn map_error(error: CryptoError) -> DependencyError {
    tracing::trace!("Mapping a cryptographic failure to the Transport dependency contract");
    match error {
        CryptoError::InvalidInput | CryptoError::ResourceLimit => DependencyError::InvalidInput,
        CryptoError::AuthenticationFailed => DependencyError::VerificationFailed,
        CryptoError::AccessDenied => DependencyError::Denied,
        CryptoError::Unavailable => DependencyError::Unavailable,
    }
}

#[cfg(test)]
mod tests;
