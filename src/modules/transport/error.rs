//! Typed immediate local admission failures; wire failures remain separate.
use std::{error::Error, fmt};

use super::{
    config::ConfigError,
    ports::{Dependency, DependencyError},
};

/// Local request failure; these variants are not CR2SE wire error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportError {
    /// Argument-free startup requires explicit protected server configuration.
    RuntimeUnavailable,
    InvalidConfig(ConfigError),
    WrongConnection,
    WrongStreamOwner,
    InvalidEndpoint,
    /// Local request violates a validated protocol or lifecycle contract.
    InvalidRequest,
    /// Operating-system socket/bind/connect failure without peer diagnostics.
    Io(std::io::ErrorKind),
    /// Startup failed before a connection could be admitted.
    Startup(super::api::Failure),
    Dependency {
        dependency: Dependency,
        error: DependencyError,
    },
    /// No work was queued; the caller may wait for capacity before resubmission.
    Backpressure,
    Closed,
}

impl fmt::Display for TransportError {
    // Explain the typed failure without exposing peer data or secret material.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuntimeUnavailable => formatter.write_str(
                "Transport startup requires explicit server configuration; use talksphere serve --identity-store DIRECTORY.",
            ),
            Self::InvalidConfig(error) => write!(formatter, "invalid Transport configuration: {error}"),
            Self::WrongConnection => formatter.write_str("stream belongs to a different connection"),
            Self::WrongStreamOwner => formatter.write_str("stream belongs to a different local owner"),
            Self::InvalidRequest => formatter.write_str("invalid Transport request or state"),
            Self::Io(kind) => write!(formatter, "Transport socket operation failed: {kind:?}"),
            Self::Startup(failure) => write!(formatter, "Transport startup failed: {:?}", failure.code),
            Self::InvalidEndpoint => formatter.write_str("outgoing connection port must be nonzero"),
            Self::Dependency { dependency, error } => write!(formatter, "{dependency:?} dependency failed: {error:?}"),
            Self::Backpressure => formatter.write_str("Transport capacity is full; request was not queued"),
            Self::Closed => formatter.write_str("Transport resource is closed"),
        }
    }
}

impl Error for TransportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidConfig(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ConfigError> for TransportError {
    fn from(error: ConfigError) -> Self {
        Self::InvalidConfig(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Argument-free startup explains explicit protected configuration without opening a listener.
    #[test]
    fn runtime_error_requires_explicit_configuration() {
        let error: &dyn Error = &TransportError::RuntimeUnavailable;
        let message = error.to_string();
        assert!(message.contains("explicit server configuration"));
        assert!(message.contains("--identity-store"));
        assert!(message.contains("talksphere serve"));
        assert!(error.source().is_none());
    }

    #[test]
    fn config_errors_preserve_machine_readable_cause() {
        let cause = ConfigError::ZeroHandshakeTimeout;
        let error = TransportError::from(cause);
        assert_eq!(error, TransportError::InvalidConfig(cause));
        assert_eq!(error.source().unwrap().to_string(), cause.to_string());
    }
}
