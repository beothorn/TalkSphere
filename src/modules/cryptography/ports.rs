//! Injected platform entropy; composition selects a production adapter later.

use super::error::CryptoError;

/// Fallible secure entropy supplied by platform composition, never by peers.
///
/// Production implementations must use a CSPRNG without deterministic fallback.
/// Test substitutes must stay in test-only code. No implementation is provided.
pub trait Entropy: Send {
    /// Fill the caller-owned bounded buffer with fresh cryptographic randomness.
    ///
    /// An empty buffer is permitted. The caller chooses a bounded request size;
    /// implementations must not log output or retain a copy.
    ///
    /// # Errors
    /// Return `Unavailable` if secure randomness cannot be obtained, never a
    /// synthetic success. On failure the buffer may be partially filled; callers
    /// must discard/erase it and must not use any of its bytes as key material.
    fn fill(&mut self, output: &mut [u8]) -> Result<(), CryptoError>;
}
