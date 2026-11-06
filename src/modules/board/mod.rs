//! Atomic offering/definition publication contracts for local composition and future peer control.
//! Uses Identity IDs and Services schema formats; execution and payment belong to other modules.

pub mod authority;
pub mod cache;
pub mod config;
pub mod contract;
pub mod definition;
pub mod document;
pub mod offering;
pub mod persistence;
pub mod prepared;
pub mod response;
pub mod semantics;
pub mod state;
pub mod terms;

#[cfg(test)]
mod test_support;

/// Expected failures at the Board boundary; diagnostics never contain untrusted documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Malformed document, inconsistent definition or invalid configuration.
    Invalid,
    /// Unknown contract version or capability not implemented.
    Unsupported,
    /// Foreign, expired or revoked local authority.
    Denied,
    /// A configured resource budget would be exceeded.
    Limit,
    /// Requested offering or snapshot does not exist.
    Missing,
    /// A selection no longer matches current publication.
    Stale,
    /// Deadline passed or local caller cancelled before effects.
    Cancelled,
    /// Persistence failed before commitment.
    Storage,
    /// Commitment may have occurred; recover before further mutation.
    Unknown,
}

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
mod record_codec;

#[cfg(test)]
#[path = "tests/recovery.rs"]
mod recovery_tests;
