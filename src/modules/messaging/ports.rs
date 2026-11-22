//! Dependencies Messaging receives from composition.
//!
//! Callers: the provider and requester engines. Production composition supplies platform
//! implementations; tests supply deterministic ones. Nothing here may block indefinitely or log secrets.
use super::{error::Error, ids::Signature};
use crate::modules::identity::api::IdentityId;

/// Provider wall-clock time in whole Unix seconds.
pub trait Clock: Send + Sync {
    /// The current Unix time.
    /// # Errors
    /// `Unavailable` when no trustworthy time exists; no operation proceeds without one.
    fn now(&self) -> Result<u64, Error>;
}

/// Cryptographically secure randomness for identifiers.
pub trait Entropy: Send + Sync {
    /// Fill `bytes` with unpredictable bytes.
    /// # Errors
    /// `Unavailable` when randomness cannot be obtained; no identifier is ever derived from a fallback.
    fn fill(&self, bytes: &mut [u8]) -> Result<(), Error>;
}

/// Signing with the provider's identity key, used only for recovery receipts.
pub trait Signer: Send + Sync {
    /// The identity that owns the signing key.
    fn identity(&self) -> IdentityId;
    /// The identity's 32-byte Ed25519 public key.
    fn public_key(&self) -> [u8; 32];
    /// Sign exactly the given bytes.
    /// # Errors
    /// `Unavailable` when the key is not accessible.
    fn sign(&self, message: &[u8]) -> Result<Signature, Error>;
}
