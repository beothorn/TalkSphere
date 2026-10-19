//! Immutable local byte budgets, validated before a cryptographic provider starts.

use super::error::CryptoError;

/// Smallest generic envelope: 132 header bytes and a 16-byte tag.
pub const MINIMUM_OBJECT_BYTES: usize = 148;

/// Validated policy; plaintext and object budgets apply independently per call.
///
/// These are local limits, not new protocol maxima. Empty plaintext remains valid.
///
/// ```
/// use talksphere::modules::cryptography::config::ResourceLimits;
/// let limits = ResourceLimits::new(1024, 1172)?;
/// assert_eq!(limits.maximum_plaintext_bytes(), 1024);
/// # Ok::<(), talksphere::modules::cryptography::error::CryptoError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLimits {
    maximum_plaintext_bytes: usize,
    maximum_object_bytes: usize,
}

impl ResourceLimits {
    /// Validate positive plaintext capacity and room for an empty envelope.
    ///
    /// Both arguments count bytes and originate in local application policy.
    /// The object budget may further restrict the permitted plaintext size.
    /// No allocation, arithmetic on declared wire lengths or I/O occurs here.
    ///
    /// # Errors
    /// Returns `InvalidInput` for zero plaintext capacity or an object budget
    /// below 148 bytes. Later operations must check their own lengths/overflow.
    pub fn new(
        maximum_plaintext_bytes: usize,
        maximum_object_bytes: usize,
    ) -> Result<Self, CryptoError> {
        tracing::trace!("Validating Cryptography resource policy");
        if maximum_plaintext_bytes == 0 || maximum_object_bytes < MINIMUM_OBJECT_BYTES {
            return Err(CryptoError::InvalidInput);
        }
        Ok(Self {
            maximum_plaintext_bytes,
            maximum_object_bytes,
        })
    }

    /// Return the local plaintext byte budget without changing it.
    pub fn maximum_plaintext_bytes(&self) -> usize {
        tracing::trace!("Reading the Cryptography plaintext budget");
        self.maximum_plaintext_bytes
    }

    /// Return the local complete-envelope byte budget, including header and tag.
    pub fn maximum_object_bytes(&self) -> usize {
        tracing::trace!("Reading the Cryptography object budget");
        self.maximum_object_bytes
    }
}
