//! Typed Messaging failures and their mapping to the common Services errors.
//!
//! Callers: every Messaging operation. A failure before durable commitment is uncharged. Errors name
//! categories only: they never carry identifiers, payload bytes or peer text, and an unauthorized
//! request is reported exactly like an unknown placement so existence is not revealed.
use crate::modules::services;

/// Why a Messaging operation or validation failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// A malformed field, value, length, signature or forbidden field for the operation.
    Invalid,
    /// An unsupported service version, pricing model, encoding or mode.
    Unsupported,
    /// The caller is not authorized, or the placement is unknown: indistinguishable on purpose.
    NotAvailable,
    /// A configured finite limit or capacity was reached.
    Limit,
    /// A stale expected expiration or a conflicting replay identifier.
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

impl From<Error> for services::Error {
    // Services has no concealed-denial variant of its own: it is `Denied`, which Peer Protocol reports as not_available.
    fn from(error: Error) -> Self {
        tracing::trace!("Mapping a Messaging error to Services");
        match error {
            Error::Invalid => Self::Invalid,
            Error::Unsupported => Self::Unsupported,
            Error::NotAvailable => Self::Denied,
            Error::Limit => Self::Limit,
            Error::Conflict => Self::Conflict,
            Error::Insufficient => Self::Insufficient,
            Error::Storage => Self::Storage,
            Error::Unknown => Self::Unknown,
            Error::Unavailable => Self::Unavailable,
        }
    }
}

impl From<services::Error> for Error {
    // Map conservatively: anything that may have had an effect stays unknown.
    fn from(error: services::Error) -> Self {
        tracing::trace!("Mapping a Services error to Messaging");
        match error {
            services::Error::Invalid => Self::Invalid,
            services::Error::Unsupported => Self::Unsupported,
            services::Error::Denied => Self::NotAvailable,
            services::Error::Limit => Self::Limit,
            services::Error::Conflict => Self::Conflict,
            services::Error::Insufficient => Self::Insufficient,
            services::Error::Storage => Self::Storage,
            services::Error::Unavailable => Self::Unavailable,
            services::Error::Cancelled
            | services::Error::EvidenceUnavailable
            | services::Error::Unknown => Self::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every Messaging error maps to the Services error with the same meaning, and the mapping round trips
    // except for the unauthorized case, which Services names `Denied`.
    #[test]
    fn errors_map_to_services_and_back() {
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
        }
        assert_eq!(
            services::Error::from(Error::NotAvailable),
            services::Error::Denied
        );
    }

    // Anything that may have had an effect, or was cancelled, stays unknown rather than becoming a safe failure.
    #[test]
    fn possible_effects_stay_unknown() {
        for error in [
            services::Error::Cancelled,
            services::Error::EvidenceUnavailable,
            services::Error::Unknown,
        ] {
            assert_eq!(Error::from(error), Error::Unknown);
        }
    }
}
