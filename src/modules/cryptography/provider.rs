//! In-process composition of implemented primitives and envelopes behind the public contracts.

use super::{
    api::{self, Hashing, IdentityEncryption, KeyAgreement, Signatures},
    config::ResourceLimits,
    envelope::{decrypt, encrypt},
    error::CryptoError,
    ports::Entropy,
    primitives::{agreement, hash, keys, signature},
    random::{SystemEntropy, random_bytes},
    secrets::{Ephemeral, Persistent, Secret, Shared, Signing},
};

/// Real cryptography with trusted injected entropy and one immutable local byte ceiling.
/// It has no persistent key store, remote authority endpoint or implicit network access.
///
/// # Example
/// Generate local credentials and authenticate an independently encrypted object.
/// Persistent credential protection remains the caller's responsibility.
/// ```
/// use talksphere::modules::cryptography::{
///     api::{DecryptRequest, EncryptRequest, IdentityEncryption, KeyAgreement,
///           RecipientCredential, Signatures},
///     config::ResourceLimits, credential, provider::Provider,
/// };
/// let mut provider = Provider::system(4096)?;
/// let signing = provider.generate_signing_key()?;
/// let encryption = provider.generate_persistent_key()?;
/// let recipient = RecipientCredential {
///     identity_id: credential::identity_id(&signing.public_key)?,
///     signing_public_key: signing.public_key,
///     encryption_public_key: encryption.public_key,
///     binding_signature: credential::bind(&signing.private_key, &encryption.public_key)?,
/// };
/// let limits = ResourceLimits::new(1024, 1172)?;
/// let object = provider.encrypt(EncryptRequest {
///     plaintext: b"local example", recipient: &recipient, limits,
/// })?;
/// let plaintext = provider.decrypt(DecryptRequest {
///     object: &object, identity_id: recipient.identity_id,
///     encryption_public_key: encryption.public_key,
///     private_key: &encryption.private_key, limits,
/// })?;
/// assert_eq!(plaintext, b"local example");
/// # Ok::<(), talksphere::modules::cryptography::error::CryptoError>(())
/// ```
pub struct Provider<E = SystemEntropy> {
    entropy: E,
    maximum_bytes: usize,
}
impl Provider<SystemEntropy> {
    /// Select secure platform entropy for normal production composition.
    /// # Errors
    /// Rejects a byte ceiling below the minimum complete envelope size.
    pub fn system(maximum_bytes: usize) -> Result<Self, CryptoError> {
        tracing::trace!("Selecting platform-backed cryptographic composition");
        Self::new(SystemEntropy, maximum_bytes)
    }
}
impl<E: Entropy> Provider<E> {
    /// Compose a trusted entropy dependency and maximum per-input/object byte budget.
    /// Deterministic entropy implementations belong only in tests.
    /// # Errors
    /// Returns `InvalidInput` below 148 bytes; construction performs no entropy requests.
    pub fn new(entropy: E, maximum_bytes: usize) -> Result<Self, CryptoError> {
        tracing::trace!("Constructing a bounded cryptographic provider");
        ResourceLimits::new(maximum_bytes, maximum_bytes)?;
        Ok(Self {
            entropy,
            maximum_bytes,
        })
    }
    /// Return the provider's immutable local byte ceiling for boundary adapters.
    pub fn maximum_bytes(&self) -> usize {
        tracing::trace!("Reading the cryptographic provider ceiling");
        self.maximum_bytes
    }
    /// Obtain a fresh public 32-byte HELLO nonce from the trusted source.
    /// # Errors
    /// Propagates secure entropy failure without returning partial randomness.
    pub fn hello_nonce(&mut self) -> Result<[u8; 32], CryptoError> {
        tracing::trace!("Generating a fresh Transport HELLO nonce");
        Ok(*random_bytes(&mut self.entropy)?)
    }
    // Intersect per-request policy with the provider ceiling without silently enlarging either.
    fn bounded(&self, limits: ResourceLimits) -> Result<ResourceLimits, CryptoError> {
        tracing::trace!("Intersecting caller and provider object budgets");
        ResourceLimits::new(
            limits.maximum_plaintext_bytes().min(self.maximum_bytes),
            limits.maximum_object_bytes().min(self.maximum_bytes),
        )
    }
}
impl<E: Entropy> Hashing for Provider<E> {
    /// Hash exact bytes within the provider ceiling, returning a resource error if exceeded.
    fn sha256(&self, bytes: &[u8]) -> Result<[u8; 32], CryptoError> {
        tracing::trace!("Serving a bounded SHA-256 request");
        hash::sha256(bytes, self.maximum_bytes)
    }
}
impl<E: Entropy> Signatures for Provider<E> {
    type SigningKey = Secret<Signing>;
    /// Generate independent local signing authority or propagate entropy failure.
    fn generate_signing_key(&mut self) -> Result<api::KeyPair<Self::SigningKey>, CryptoError> {
        tracing::trace!("Serving signing-key generation");
        keys::signing_key(&mut self.entropy)
    }
    /// Sign authorized exact bytes within the local ceiling or return a resource error.
    fn sign(&self, key: &Self::SigningKey, message: &[u8]) -> Result<[u8; 64], CryptoError> {
        tracing::trace!("Serving an authorized signing request");
        signature::sign(key, message, self.maximum_bytes)
    }
    /// Verify exact signatures with the documented strict policy and local limits.
    fn verify(
        &self,
        public_key: &[u8; 32],
        message: &[u8],
        signature: &[u8; 64],
    ) -> Result<(), CryptoError> {
        tracing::trace!("Serving an Ed25519 verification request");
        signature::verify(public_key, message, signature, self.maximum_bytes)
    }
}
impl<E: Entropy> KeyAgreement for Provider<E> {
    type PersistentKey = Secret<Persistent>;
    type EphemeralKey = Secret<Ephemeral>;
    type SharedSecret = Secret<Shared>;
    /// Generate persistent authority without installing credentials; entropy failures propagate.
    fn generate_persistent_key(
        &mut self,
    ) -> Result<api::KeyPair<Self::PersistentKey>, CryptoError> {
        tracing::trace!("Serving persistent-key generation");
        keys::persistent_key(&mut self.entropy)
    }
    /// Generate fresh ephemeral authority or return entropy failure.
    fn generate_ephemeral_key(&mut self) -> Result<api::KeyPair<Self::EphemeralKey>, CryptoError> {
        tracing::trace!("Serving ephemeral-key generation");
        keys::ephemeral_key(&mut self.entropy)
    }
    /// Consume ephemeral authority and reject non-contributory agreement results.
    fn agree_ephemeral(
        &self,
        key: Self::EphemeralKey,
        remote_public_key: &[u8; 32],
    ) -> Result<Self::SharedSecret, CryptoError> {
        tracing::trace!("Serving ephemeral agreement");
        agreement::ephemeral(key, remote_public_key)
    }
    /// Borrow persistent authority and reject non-contributory agreement results.
    fn agree_persistent(
        &self,
        key: &Self::PersistentKey,
        remote_public_key: &[u8; 32],
    ) -> Result<Self::SharedSecret, CryptoError> {
        tracing::trace!("Serving persistent agreement");
        agreement::persistent(key, remote_public_key)
    }
}
impl<E: Entropy> IdentityEncryption for Provider<E> {
    type PersistentKey = Secret<Persistent>;
    /// Encrypt a bounded complete object; credential, entropy and primitive errors propagate.
    fn encrypt(&mut self, mut request: api::EncryptRequest<'_>) -> Result<Vec<u8>, CryptoError> {
        tracing::trace!("Serving recipient encryption through the public contract");
        request.limits = self.bounded(request.limits)?;
        encrypt::encrypt(request, &mut self.entropy)
    }
    /// Authenticate a complete object before returning plaintext, preserving all failure rules.
    fn decrypt(
        &self,
        mut request: api::DecryptRequest<'_, Self::PersistentKey>,
    ) -> Result<Vec<u8>, CryptoError> {
        tracing::trace!("Serving recipient decryption through the public contract");
        request.limits = self.bounded(request.limits)?;
        decrypt::decrypt(request)
    }
}

#[cfg(test)]
mod tests;
