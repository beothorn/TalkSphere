//! Typed primitive and envelope contracts for authorized in-process callers.
//!
//! These traits define capabilities implemented by the real Provider. Implementations
//! must bound inputs, log entry without secret fields and erase transient secrets
//! on every exit. No trait requires private material to be Clone, Debug or bytes.
//! Persistent key authorization and storage belong to Identity/key-store adapters.

use super::{config::ResourceLimits, error::CryptoError};

/// Public bytes paired with an opaque, purpose-specific private capability.
/// No serialization or diagnostic implementation exposes the private value.
pub struct KeyPair<K> {
    /// Canonical 32-byte public key for the capability's algorithm.
    pub public_key: [u8; 32],
    /// Owned authorized capability; its provider must release secrets on drop.
    pub private_key: K,
}

/// Untrusted recipient credential, requiring ID and binding verification.
/// The ID is SHA-256(0x01 || 0x01 || signing public key), without an Identity call.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RecipientCredential {
    /// Claimed version-1 identity digest, not proof of identity or trust.
    pub identity_id: [u8; 32],
    /// Ed25519 public key used to verify the binding signature.
    pub signing_public_key: [u8; 32],
    /// Persistent X25519 recipient public key.
    pub encryption_public_key: [u8; 32],
    /// Ordinary Ed25519 signature over the specified key-binding bytes.
    pub binding_signature: [u8; 64],
}

/// Borrowed encryption inputs; no external associated data is part of this format.
pub struct EncryptRequest<'a> {
    /// Exact bounded application bytes, which must not appear in diagnostics.
    pub plaintext: &'a [u8],
    /// Untrusted credential to validate before generating ephemeral material.
    pub recipient: &'a RecipientCredential,
    /// Local budgets to check before allocating the result.
    pub limits: ResourceLimits,
}

/// Borrowed local decryption inputs; possession of the private capability grants use.
pub struct DecryptRequest<'a, K> {
    /// Complete header, ciphertext and tag, with no trailing bytes permitted.
    pub object: &'a [u8],
    /// Expected local identity, compared before agreement.
    pub identity_id: [u8; 32],
    /// Expected local persistent public key, compared before agreement.
    pub encryption_public_key: [u8; 32],
    /// Authorized persistent capability, borrowed without transferring ownership.
    pub private_key: &'a K,
    /// Local budgets enforced before size-dependent work.
    pub limits: ResourceLimits,
}

/// SHA-256 operations over exact input bytes; no implicit serialization.
pub trait Hashing {
    /// Hash bounded bytes to a 32-byte digest.
    ///
    /// # Errors
    /// Return `ResourceLimit` for excessive input or `Unavailable` for a missing backend.
    fn sha256(&self, bytes: &[u8]) -> Result<[u8; 32], CryptoError>;
}

/// Ordinary Ed25519 with private authority supplied by local composition.
pub trait Signatures {
    /// Provider-owned signing authority, with no required extraction or copying.
    type SigningKey: Send;

    /// Generate an independent signing pair using the provider's secure entropy.
    ///
    /// # Errors
    /// Propagate entropy/key-store failure and release partially created secrets.
    fn generate_signing_key(&mut self) -> Result<KeyPair<Self::SigningKey>, CryptoError>;

    /// Sign exact bounded domain-specific bytes through an authorized capability.
    /// Callers must not expose this as an unrestricted peer signing service.
    ///
    /// # Errors
    /// Return `AccessDenied`, `ResourceLimit` or `Unavailable` without secret details.
    fn sign(&self, key: &Self::SigningKey, message: &[u8]) -> Result<[u8; 64], CryptoError>;

    /// Verify ordinary Ed25519 over exact bytes; successful verification grants no trust.
    ///
    /// # Errors
    /// Return `InvalidInput` for malformed values, `AuthenticationFailed` for a
    /// rejected signature, or the relevant resource/dependency error.
    fn verify(
        &self,
        public_key: &[u8; 32],
        message: &[u8],
        signature: &[u8; 64],
    ) -> Result<(), CryptoError>;
}

/// X25519 capabilities separate persistent borrowing from ephemeral consumption.
///
/// Ephemeral authority cannot be reused by a generic caller after agreement:
/// ```compile_fail
/// use talksphere::modules::cryptography::api::KeyAgreement;
/// fn reuse<C: KeyAgreement>(crypto: &C, key: C::EphemeralKey) {
///     let _ = crypto.agree_ephemeral(key, &[0; 32]);
///     let _ = crypto.agree_ephemeral(key, &[0; 32]);
/// }
/// ```
///
/// A generic caller cannot substitute an agreement capability for signing authority:
/// ```compile_fail
/// use talksphere::modules::cryptography::api::{KeyAgreement, Signatures};
/// fn misuse<C: KeyAgreement + Signatures>(crypto: &C, key: &C::EphemeralKey) {
///     let _ = crypto.sign(key, b"unauthorized");
/// }
/// ```
pub trait KeyAgreement {
    /// Persistent encryption authority borrowed across independent objects.
    type PersistentKey: Send;
    /// Single-use ephemeral agreement authority consumed by agreement.
    type EphemeralKey: Send;
    /// Protected result requiring derivation before encryption use.
    type SharedSecret: Send;

    /// Generate an independent persistent X25519 pair without persisting it here.
    ///
    /// # Errors
    /// Propagate entropy/key-store failures with cleanup of temporary material.
    fn generate_persistent_key(&mut self) -> Result<KeyPair<Self::PersistentKey>, CryptoError>;

    /// Generate fresh ephemeral X25519 material for one construction.
    ///
    /// # Errors
    /// Propagate secure entropy failure without deterministic fallback.
    fn generate_ephemeral_key(&mut self) -> Result<KeyPair<Self::EphemeralKey>, CryptoError>;

    /// Consume ephemeral authority and reject invalid/all-zero agreement results.
    ///
    /// # Errors
    /// Return authentication, access or dependency failure and erase consumed material.
    fn agree_ephemeral(
        &self,
        key: Self::EphemeralKey,
        remote_public_key: &[u8; 32],
    ) -> Result<Self::SharedSecret, CryptoError>;

    /// Borrow persistent authority for agreement without destroying the stored key.
    ///
    /// # Errors
    /// Reject invalid/all-zero results and denied or unavailable key access.
    fn agree_persistent(
        &self,
        key: &Self::PersistentKey,
        remote_public_key: &[u8; 32],
    ) -> Result<Self::SharedSecret, CryptoError>;
}

/// Generic single-object encryption; Identity persistence and trust are external.
/// Results are owned exact envelope/plaintext bytes, never provisional output.
pub trait IdentityEncryption {
    /// Protected local persistent X25519 authority, not a peer-provided handle.
    type PersistentKey: Send;

    /// Validate the recipient and encrypt with fresh ephemeral material and nonce.
    /// Self-encryption uses this same operation. Successful encryption proves no sender.
    ///
    /// # Errors
    /// Reject malformed credentials, invalid binding/ID, excessive lengths and
    /// failed dependencies with no partial object; erase temporary secrets.
    fn encrypt(&mut self, request: EncryptRequest<'_>) -> Result<Vec<u8>, CryptoError>;

    /// Authenticate a complete bounded object before releasing owned plaintext.
    /// The private capability stays borrowed; no stream of provisional bytes is exposed.
    ///
    /// # Errors
    /// Reject format, length, recipient, agreement or tag failure without plaintext.
    /// Missing implementations must return `Unavailable`, never synthetic success.
    fn decrypt(
        &self,
        request: DecryptRequest<'_, Self::PersistentKey>,
    ) -> Result<Vec<u8>, CryptoError>;
}
