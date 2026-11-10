//! Peer Control wire exchange ownership over authenticated Transport and public module contracts.
//! Composition supplies local authority; codecs and dispatch are implemented in subsequent steps.
pub mod authority;
pub mod contract;

/// Typed boundary failures; uncertainty never establishes rollback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Malformed value or binding.
    Invalid,
    /// Unsupported local/wire capability.
    Unsupported,
    /// Foreign, expired or revoked authority.
    Denied,
    /// Exhausted finite capacity.
    Limit,
    /// Caller cancellation or elapsed deadline.
    Cancelled,
    /// Conflicting operation binding.
    Conflict,
    /// Insufficient finite economic capacity.
    Insufficient,
    /// Known precommit storage refusal.
    Storage,
    /// Effects may have committed; consult owner status.
    Unknown,
    /// Required dependency unavailable.
    Unavailable,
}
