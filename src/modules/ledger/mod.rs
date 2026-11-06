//! Local Ledger contracts for composition, Services and authenticated control adapters.
//! Accounting and persistence transitions are implemented in subsequent plan steps.
pub mod accounting;
pub mod authority;
pub mod contract;
pub mod persistence;
pub mod state;
pub mod terms;

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
}

#[cfg(test)]
mod test_support;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
mod record_codec;
