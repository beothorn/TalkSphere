//! Primitive contracts only; Transport owns transcripts and session sequencing.
//!
//! Associated secret types are opaque, purpose-scoped capabilities. Adapters
//! must erase/release their material on Drop, must not expose it in diagnostics,
//! and must not replace a failed primitive with a synthetic successful result.

use super::DependencyError;

pub struct EphemeralKey<K> {
    pub public_key: [u8; 32],
    pub private_key: K,
}

pub trait Cryptography: Send {
    type EphemeralSecret: Send;
    type SharedSecret: Send;
    type PseudorandomKey: Send;
    type IntegrityKey: Send;

    /// Fresh CSPRNG bytes for each HELLO; deterministic sources are test-only.
    fn random_nonce(&mut self) -> Result<[u8; 32], DependencyError>;
    /// Fresh X25519 key material; the private capability must not be reused.
    fn ephemeral_key(&mut self) -> Result<EphemeralKey<Self::EphemeralSecret>, DependencyError>;
    fn sha256(&self, bytes: &[u8]) -> Result<[u8; 32], DependencyError>;
    fn verify_signature(
        &self,
        public_key: &[u8; 32],
        message: &[u8],
        signature: &[u8; 64],
    ) -> Result<(), DependencyError>;
    /// Consume the ephemeral capability and reject an all-zero shared result.
    fn agree(
        &mut self,
        private_key: Self::EphemeralSecret,
        remote_key: &[u8; 32],
    ) -> Result<Self::SharedSecret, DependencyError>;
    /// HKDF-Extract-SHA-256 with the exact Transport salt.
    fn extract(
        &mut self,
        salt: &[u8; 32],
        shared: Self::SharedSecret,
    ) -> Result<Self::PseudorandomKey, DependencyError>;
    /// HKDF-Expand-SHA-256 to an opaque 32-byte integrity key.
    fn expand_key(
        &mut self,
        key: &Self::PseudorandomKey,
        info: &[u8],
    ) -> Result<Self::IntegrityKey, DependencyError>;
    /// HKDF-Expand-SHA-256 to a 16-byte nonce prefix.
    fn expand_nonce_prefix(
        &self,
        key: &Self::PseudorandomKey,
        info: &[u8],
    ) -> Result<[u8; 16], DependencyError>;
    /// XChaCha20-Poly1305 with empty plaintext; header/payload are associated data.
    fn tag(
        &self,
        key: &Self::IntegrityKey,
        nonce: &[u8; 24],
        authenticated_data: &[u8],
    ) -> Result<[u8; 16], DependencyError>;
    /// Constant-time verification of the same empty-plaintext construction.
    fn verify_tag(
        &self,
        key: &Self::IntegrityKey,
        nonce: &[u8; 24],
        authenticated_data: &[u8],
        tag: &[u8; 16],
    ) -> Result<(), DependencyError>;
}
