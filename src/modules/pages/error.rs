//! Typed Static Pages failures and their mapping to the common Services errors.
//!
//! Callers: every Pages operation and the hosted page assembly. A failure before durable commitment is uncharged. Errors name categories
//! only: they never carry paths, content or peer text.
use crate::modules::{hosted_publications, services};

/// Why a Pages validation or operation failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// A malformed path, media type, manifest, field or value.
    Invalid,
    /// An unsupported service version, pricing model or kind.
    Unsupported,
    /// The resource or page is unknown or not readable: indistinguishable on purpose.
    NotAvailable,
    /// A configured finite limit was exceeded.
    Limit,
    /// A durable write or read failed.
    Storage,
    /// A required local dependency is unavailable.
    Unavailable,
}

impl From<hosted_publications::Error> for Error {
    // Hosted publication failures keep their meaning; anything that may have had an effect cannot occur in read-side assembly.
    fn from(error: hosted_publications::Error) -> Self {
        tracing::trace!("Mapping a hosted publication error to Pages");
        match error {
            hosted_publications::Error::Invalid | hosted_publications::Error::Conflict => {
                Self::Invalid
            }
            hosted_publications::Error::Unsupported => Self::Unsupported,
            hosted_publications::Error::NotAvailable => Self::NotAvailable,
            hosted_publications::Error::Limit => Self::Limit,
            hosted_publications::Error::Storage | hosted_publications::Error::Unknown => {
                Self::Storage
            }
            hosted_publications::Error::Insufficient | hosted_publications::Error::Unavailable => {
                Self::Unavailable
            }
        }
    }
}

impl From<Error> for services::Error {
    // Services has no concealed-denial variant of its own: it is `Denied`, which Peer Protocol reports as not_available.
    fn from(error: Error) -> Self {
        tracing::trace!("Mapping a Pages error to Services");
        match error {
            Error::Invalid => Self::Invalid,
            Error::Unsupported => Self::Unsupported,
            Error::NotAvailable => Self::Denied,
            Error::Limit => Self::Limit,
            Error::Storage => Self::Storage,
            Error::Unavailable => Self::Unavailable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Hosted publication failures keep their category in Pages, and Pages failures map to the matching Services error.
    #[test]
    fn errors_map_between_modules() {
        use hosted_publications::Error as Hosted;
        assert_eq!(Error::from(Hosted::Invalid), Error::Invalid);
        assert_eq!(Error::from(Hosted::Conflict), Error::Invalid);
        assert_eq!(Error::from(Hosted::NotAvailable), Error::NotAvailable);
        assert_eq!(Error::from(Hosted::Limit), Error::Limit);
        assert_eq!(Error::from(Hosted::Unknown), Error::Storage);
        assert_eq!(Error::from(Hosted::Insufficient), Error::Unavailable);
        assert_eq!(Error::from(Hosted::Unsupported), Error::Unsupported);
        assert_eq!(
            services::Error::from(Error::NotAvailable),
            services::Error::Denied
        );
        assert_eq!(services::Error::from(Error::Limit), services::Error::Limit);
        assert_eq!(
            services::Error::from(Error::Storage),
            services::Error::Storage
        );
        assert_eq!(
            services::Error::from(Error::Unavailable),
            services::Error::Unavailable
        );
        assert_eq!(
            services::Error::from(Error::Invalid),
            services::Error::Invalid
        );
        assert_eq!(
            services::Error::from(Error::Unsupported),
            services::Error::Unsupported
        );
    }
}
