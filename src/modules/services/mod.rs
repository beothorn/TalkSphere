//! Services validates typed contracts for Board and coordinates authorized family work.
//! Composition owns grants and dependencies; network dispatch remains Peer Protocol work.
pub mod authority;
pub mod contract;
pub mod schema;

/// Typed local failures, preserving uncertainty separately from explicit refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Invalid shape, value or binding.
    Invalid,
    /// Unsupported contract or handler.
    Unsupported,
    /// Missing, foreign or revoked authority.
    Denied,
    /// Finite admission budget exhausted.
    Limit,
    /// Call cancelled or deadline elapsed before effects.
    Cancelled,
    /// An operation key was reused with different parameters.
    Conflict,
    /// Authorized finite capacity is insufficient.
    Insufficient,
    /// Known failure before durable commitment.
    Storage,
    /// Commitment may have occurred; recover through status.
    Unknown,
    /// A required local dependency is unavailable.
    Unavailable,
}

#[cfg(test)]
mod test_support;

pub mod value;
