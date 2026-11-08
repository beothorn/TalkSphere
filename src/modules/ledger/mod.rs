//! Local Ledger contracts for composition, Services and authenticated control adapters.
//! Accounting and persistence transitions are implemented in subsequent plan steps.
pub mod accounting;
pub mod authority;
pub mod claim;
pub mod claims;
pub mod contract;
pub mod fees;
pub mod payment;
pub mod persistence;
pub mod redemption;
pub mod reservation;
pub mod settlement;
pub mod state;
pub mod terms;
pub mod transfer;
pub mod transfer_crypto;
pub mod transfer_state;

/// Structured local failures; diagnostic strings never determine protocol behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Invalid shape or exact value.
    Invalid,
    /// Unsupported local contract or capability.
    Unsupported,
    /// Missing or mismatching local authority.
    Denied,
    /// Finite configured resource budget exhausted.
    Limit,
    /// Cancellation or expired monotonic deadline before effects.
    Cancelled,
    /// Exact operation key reused with different terms.
    Conflict,
    /// Insufficient finite spending or issuance capacity.
    Insufficient,
    /// Known precommit persistence failure.
    Storage,
    /// An effect may be durable; recover before further dependent work.
    Unknown,
    /// A required protected or entropy dependency is unavailable before confirmed commitment.
    Unavailable,
}

#[cfg(test)]
mod test_support;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
mod record_codec;

pub mod maintenance;

#[cfg(test)]
#[path = "tests/races.rs"]
mod races;

impl From<crate::modules::cryptography::error::CryptoError> for Error {
    // Preserve local dependency/resource failures separately from rejected peer cryptographic proof.
    fn from(error: crate::modules::cryptography::error::CryptoError) -> Self {
        tracing::trace!("Mapping secret-free cryptographic dependency failure");
        use crate::modules::cryptography::error::CryptoError as C;
        match error {
            C::InvalidInput => Self::Invalid,
            C::AuthenticationFailed | C::AccessDenied => Self::Denied,
            C::Unavailable => Self::Unavailable,
            C::ResourceLimit => Self::Limit,
        }
    }
}
impl From<crate::modules::identity::error::IdentityError> for Error {
    // A possibly committed Identity operation remains unknown; no conversion asserts rollback.
    fn from(error: crate::modules::identity::error::IdentityError) -> Self {
        tracing::trace!("Mapping secret-free Identity dependency failure");
        use crate::modules::identity::error::IdentityError as I;
        match error {
            I::InvalidInput => Self::Invalid,
            I::Unsupported => Self::Unsupported,
            I::IdentityMismatch | I::InvalidProof | I::AccessDenied => Self::Denied,
            I::Unavailable => Self::Unavailable,
            I::ResourceLimit => Self::Limit,
            I::Conflict => Self::Conflict,
            I::OutcomeUnknown => Self::Unknown,
        }
    }
}

#[cfg(test)]
#[path = "tests/privacy.rs"]
mod privacy;
#[cfg(test)]
#[path = "tests/protected_adapters.rs"]
mod protected_adapters;
#[cfg(test)]
#[path = "tests/security.rs"]
mod security;

pub mod inspection;
