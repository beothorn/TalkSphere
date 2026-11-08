//! Services validates typed contracts for Board and coordinates authorized family work.
//! Composition owns grants and dependencies; network dispatch remains Peer Protocol work.
pub mod accounting;
pub mod adapter;
pub mod assessment;
pub mod authority;
pub mod authorization;
pub mod body;
pub mod checks;
pub mod codec;
pub mod contract;
pub mod handler;
pub mod journal;
pub mod persistence;
pub mod pricing;
pub mod quote;
pub mod registry;
pub mod runtime;
pub mod schema;
pub mod validation;
pub mod value;

#[cfg(target_os = "linux")]
pub mod linux;
mod operation;
#[cfg(target_os = "linux")]
mod record_codec;

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

#[cfg(test)]
#[path = "tests/validation.rs"]
mod validation_tests;

#[cfg(test)]
#[path = "tests/handler.rs"]
mod handler_tests;

#[cfg(test)]
#[path = "tests/registry.rs"]
mod registry_tests;

#[cfg(test)]
#[path = "tests/selection.rs"]
mod selection_tests;

#[cfg(test)]
#[path = "tests/authorization.rs"]
mod authorization_tests;

#[cfg(test)]
#[path = "tests/assessment.rs"]
mod assessment_tests;

#[cfg(test)]
#[path = "tests/quote.rs"]
mod quote_tests;

#[cfg(test)]
#[path = "tests/codec.rs"]
mod codec_tests;

#[cfg(test)]
#[path = "tests/journal.rs"]
mod journal_tests;

#[cfg(test)]
#[path = "tests/operation.rs"]
mod operation_tests;

#[cfg(test)]
#[path = "tests/runtime.rs"]
mod runtime_tests;

#[cfg(test)]
#[path = "tests/recovery.rs"]
mod recovery_tests;

#[cfg(test)]
#[path = "tests/checks.rs"]
mod check_tests;

#[cfg(test)]
#[path = "tests/lifecycle.rs"]
mod lifecycle_tests;

#[cfg(test)]
#[path = "tests/privacy.rs"]
mod privacy_tests;
