//! Typed Hosted Publications failures and their mapping to the common Services errors.
//!
//! Callers: every Hosted Publications operation. A failure before durable commitment is uncharged. Errors name categories only:
//! they never carry identifiers, content or peer text, and an unauthorized request is reported exactly like an unknown
//! publication so existence is not revealed.
use crate::modules::{messaging, services};

/// Why a Hosted Publications operation or validation failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// A malformed field, value, length, signature, envelope or a forbidden field for the operation.
    Invalid,
    /// An unsupported service version, pricing model or kind.
    Unsupported,
    /// The caller may not use the item, or it is unknown: indistinguishable on purpose.
    NotAvailable,
    /// A configured finite limit or capacity was reached.
    Limit,
    /// A stale expectation, duplicate publication or conflicting sequence or replay identifier.
    Conflict,
    /// The authorized finite credits cannot cover the operation.
    Insufficient,
    /// A durable write or read failed before commitment.
    Storage,
    /// The outcome may have been committed; recover through status, never retry blindly.
    Unknown,
    /// A required local dependency (clock, entropy, signer) is unavailable.
    Unavailable,
}

impl From<messaging::Error> for Error {
    // Messaging's categories have identical meanings, so the reused pricing and port helpers map one to one.
    fn from(error: messaging::Error) -> Self {
        tracing::trace!("Mapping a Messaging helper error to Hosted Publications");
        match error {
            messaging::Error::Invalid => Self::Invalid,
            messaging::Error::Unsupported => Self::Unsupported,
            messaging::Error::NotAvailable => Self::NotAvailable,
            messaging::Error::Limit => Self::Limit,
            messaging::Error::Conflict => Self::Conflict,
            messaging::Error::Insufficient => Self::Insufficient,
            messaging::Error::Storage => Self::Storage,
            messaging::Error::Unknown => Self::Unknown,
            messaging::Error::Unavailable => Self::Unavailable,
        }
    }
}

impl From<Error> for messaging::Error {
    // The reverse mapping lets reused Messaging ports (ledger issuer, release policy) report through this module.
    fn from(error: Error) -> Self {
        tracing::trace!("Mapping a Hosted Publications error to Messaging");
        match error {
            Error::Invalid => Self::Invalid,
            Error::Unsupported => Self::Unsupported,
            Error::NotAvailable => Self::NotAvailable,
            Error::Limit => Self::Limit,
            Error::Conflict => Self::Conflict,
            Error::Insufficient => Self::Insufficient,
            Error::Storage => Self::Storage,
            Error::Unknown => Self::Unknown,
            Error::Unavailable => Self::Unavailable,
        }
    }
}

impl From<Error> for services::Error {
    // Services has no concealed-denial variant of its own: it is `Denied`, which Peer Protocol reports as not_available.
    fn from(error: Error) -> Self {
        tracing::trace!("Mapping a Hosted Publications error to Services");
        services::Error::from(messaging::Error::from(error))
    }
}

impl From<services::Error> for Error {
    // Map conservatively: anything that may have had an effect stays unknown.
    fn from(error: services::Error) -> Self {
        tracing::trace!("Mapping a Services error to Hosted Publications");
        Self::from(messaging::Error::from(error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every error maps to Services and back unchanged, except the concealed denial that Services names `Denied`.
    #[test]
    fn errors_round_trip_through_services_and_messaging() {
        for error in [
            Error::Invalid,
            Error::Unsupported,
            Error::NotAvailable,
            Error::Limit,
            Error::Conflict,
            Error::Insufficient,
            Error::Storage,
            Error::Unknown,
            Error::Unavailable,
        ] {
            assert_eq!(Error::from(services::Error::from(error)), error);
            assert_eq!(Error::from(messaging::Error::from(error)), error);
        }
        assert_eq!(
            services::Error::from(Error::NotAvailable),
            services::Error::Denied
        );
    }
}
