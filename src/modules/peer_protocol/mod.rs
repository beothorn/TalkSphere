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

#[cfg(test)]
mod test_support;

pub mod record;
#[cfg(test)]
#[path = "tests/record.rs"]
mod record_tests;

pub mod outbound;
#[cfg(test)]
#[path = "tests/outbound.rs"]
mod outbound_tests;

pub mod control;
#[cfg(test)]
#[path = "tests/control.rs"]
mod control_tests;

pub mod base64;
pub mod message;
#[cfg(test)]
#[path = "tests/message.rs"]
mod message_tests;
pub mod scalar;

#[cfg(test)]
#[path = "tests/scalar.rs"]
mod scalar_tests;
