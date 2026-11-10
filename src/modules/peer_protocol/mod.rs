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

pub mod inline;
#[cfg(test)]
#[path = "tests/inline.rs"]
mod inline_tests;

pub mod value;

#[cfg(test)]
#[path = "tests/value.rs"]
mod value_tests;

impl From<crate::modules::services::Error> for Error {
    // Preserve dependency failures exactly; Unknown must never become an explicit uncharged refusal.
    fn from(error: crate::modules::services::Error) -> Self {
        tracing::trace!("Mapping Services failure at the Peer Protocol boundary");
        use crate::modules::services::Error as Source;
        match error {
            Source::Invalid => Self::Invalid,
            Source::Unsupported => Self::Unsupported,
            Source::Denied => Self::Denied,
            Source::Limit => Self::Limit,
            Source::Cancelled => Self::Cancelled,
            Source::Conflict => Self::Conflict,
            Source::Insufficient => Self::Insufficient,
            Source::Storage => Self::Storage,
            Source::Unknown => Self::Unknown,
            Source::Unavailable => Self::Unavailable,
        }
    }
}

pub mod receiver;
#[cfg(test)]
#[path = "tests/receiver.rs"]
mod receiver_tests;

pub mod sender;
#[cfg(test)]
#[path = "tests/sender.rs"]
mod sender_tests;

pub mod exchange;
#[cfg(test)]
#[path = "tests/exchange.rs"]
mod exchange_tests;

pub mod metadata;
#[cfg(test)]
#[path = "tests/metadata.rs"]
mod metadata_tests;

pub mod metadata_client;
impl From<crate::modules::board::Error> for Error {
    // Preserve uncertainty and distinguish stale selections from unavailable metadata at the local boundary.
    fn from(error: crate::modules::board::Error) -> Self {
        tracing::trace!("Mapping Board failure at the Peer Protocol boundary");
        use crate::modules::board::Error as Source;
        match error {
            Source::Invalid => Self::Invalid,
            Source::Unsupported => Self::Unsupported,
            Source::Denied => Self::Denied,
            Source::Limit => Self::Limit,
            Source::Missing => Self::Unavailable,
            Source::Stale => Self::Conflict,
            Source::Cancelled => Self::Cancelled,
            Source::Storage => Self::Storage,
            Source::Unknown => Self::Unknown,
        }
    }
}

#[cfg(test)]
#[path = "tests/metadata_client.rs"]
mod metadata_client_tests;
