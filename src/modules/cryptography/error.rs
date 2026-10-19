//! Secret-free failures returned to local callers by cryptographic boundaries.

use std::{error::Error, fmt};

/// A local failure category, not a Transport wire error or diagnostic payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoError {
    /// Malformed bytes, sizes, keys or unsupported algorithm parameters.
    InvalidInput,
    /// A signature, recipient binding, agreement result or tag was rejected.
    AuthenticationFailed,
    /// The caller lacks authority to use the requested private capability.
    AccessDenied,
    /// Entropy, a protected key store or a primitive is unavailable/unimplemented.
    Unavailable,
    /// Checked arithmetic or a configured local resource budget was exceeded.
    ResourceLimit,
}

impl fmt::Display for CryptoError {
    // Return a fixed explanation without interpolating keys or peer input.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidInput => "invalid cryptographic input",
            Self::AuthenticationFailed => "cryptographic authentication failed",
            Self::AccessDenied => "cryptographic key access denied",
            Self::Unavailable => "cryptographic dependency unavailable",
            Self::ResourceLimit => "cryptographic resource limit exceeded",
        })
    }
}

impl Error for CryptoError {}
