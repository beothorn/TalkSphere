use std::{error::Error, fmt};

use super::{
    config::ConfigError,
    ports::{Dependency, DependencyError},
};

/// Local request failure; these variants are not CR2SE wire error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportError {
    /// The public boundary is defined, but networking is not implemented.
    NotImplemented,
    InvalidConfig(ConfigError),
    WrongConnection,
    WrongStreamOwner,
    InvalidEndpoint,
    Dependency {
        dependency: Dependency,
        error: DependencyError,
    },
    /// No work was queued; the caller may wait for capacity before resubmission.
    Backpressure,
    Closed,
}

impl fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotImplemented => formatter.write_str(
                "Transport networking is not implemented yet; no server was started. See plans/1_Transport_module/PLAN.md.",
            ),
            Self::InvalidConfig(error) => write!(formatter, "invalid Transport configuration: {error}"),
            Self::WrongConnection => formatter.write_str("stream belongs to a different connection"),
            Self::WrongStreamOwner => formatter.write_str("stream belongs to a different local owner"),
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

    #[test]
    fn scaffold_error_explains_the_failure_and_next_step() {
        let error: &dyn Error = &TransportError::NotImplemented;
        let message = error.to_string();
        assert!(message.contains("not implemented"));
        assert!(message.contains("no server was started"));
        assert!(message.contains("plans/1_Transport_module/PLAN.md"));
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
