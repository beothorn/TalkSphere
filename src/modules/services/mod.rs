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

pub mod validation;
pub mod value;

#[cfg(test)]
#[path = "tests/validation.rs"]
mod validation_tests;

pub mod handler;
#[cfg(test)]
#[path = "tests/handler.rs"]
mod handler_tests;

pub mod registry;
#[cfg(test)]
#[path = "tests/registry.rs"]
mod registry_tests;
#[cfg(test)]
#[path = "tests/selection.rs"]
mod selection_tests;

pub mod authorization;
#[cfg(test)]
#[path = "tests/authorization.rs"]
mod authorization_tests;

pub mod assessment;
#[cfg(test)]
#[path = "tests/assessment.rs"]
mod assessment_tests;

pub mod pricing;

pub mod quote;
#[cfg(test)]
#[path = "tests/quote.rs"]
mod quote_tests;

pub mod codec;
#[cfg(test)]
#[path = "tests/codec.rs"]
mod codec_tests;
pub mod journal;
#[cfg(test)]
#[path = "tests/journal.rs"]
mod journal_tests;
pub mod persistence;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
mod record_codec;

pub mod accounting;
mod operation;
#[cfg(test)]
#[path = "tests/operation.rs"]
mod operation_tests;
pub mod runtime;
#[cfg(test)]
#[path = "tests/runtime.rs"]
mod runtime_tests;

pub mod body;
#[cfg(test)]
#[path = "tests/recovery.rs"]
mod recovery_tests;

#[cfg(test)]
#[path = "tests/checks.rs"]
mod check_tests;
pub mod checks;
