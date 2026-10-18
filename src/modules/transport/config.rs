//! Finite local policy validated before driver admission and queue enforcement.

use std::{error::Error, fmt, time::Duration};

pub use super::codec::MAX_PAYLOAD_BYTES;
use super::codec::{HEADER_BYTES, INTEGRITY_TAG_BYTES};
/// Space for one maximum protected frame: header, payload and integrity tag.
pub const MAX_FRAME_BYTES: usize = HEADER_BYTES + MAX_PAYLOAD_BYTES + INTEGRITY_TAG_BYTES;

/// Finite local budgets; byte budgets count bytes, not queue entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLimits {
    pub connections: usize,
    pub streams_per_connection: usize,
    /// Total serialized outbound frame bytes across this Transport instance.
    pub outbound_bytes: usize,
    /// Verified incoming payload bytes awaiting a single stream consumer.
    pub buffered_bytes_per_stream: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    Connections,
    StreamsPerConnection,
    OutboundBytes,
    BufferedBytesPerStream,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    ZeroHandshakeTimeout,
    LimitTooSmall { limit: Limit, minimum: usize },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroHandshakeTimeout => f.write_str("handshake timeout must be positive"),
            Self::LimitTooSmall { limit, minimum } => {
                write!(f, "{limit:?} must be at least {minimum}")
            }
        }
    }
}

impl Error for ConfigError {}

/// Validated immutable policy shared by the connection machinery.
///
/// Construction performs no I/O and does not start Transport.
///
/// ```
/// use std::time::Duration;
/// use talksphere::modules::transport::config::{
///     MAX_FRAME_BYTES, MAX_PAYLOAD_BYTES, ResourceLimits, TransportConfig,
/// };
/// let config = TransportConfig::new(ResourceLimits {
///     connections: 4,
///     streams_per_connection: 8,
///     outbound_bytes: 4 * MAX_FRAME_BYTES,
///     buffered_bytes_per_stream: MAX_PAYLOAD_BYTES,
/// }, Duration::from_secs(10))?;
/// assert_eq!(config.limits().connections, 4);
/// # Ok::<(), talksphere::modules::transport::config::ConfigError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportConfig {
    limits: ResourceLimits,
    handshake_timeout: Duration,
}

impl TransportConfig {
    /// Validate policy without reducing the version 1 maximum frame size.
    pub fn new(limits: ResourceLimits, handshake_timeout: Duration) -> Result<Self, ConfigError> {
        if handshake_timeout.is_zero() {
            return Err(ConfigError::ZeroHandshakeTimeout);
        }
        for (limit, value, minimum) in [
            (Limit::Connections, limits.connections, 1),
            (
                Limit::StreamsPerConnection,
                limits.streams_per_connection,
                1,
            ),
            (Limit::OutboundBytes, limits.outbound_bytes, MAX_FRAME_BYTES),
            (
                Limit::BufferedBytesPerStream,
                limits.buffered_bytes_per_stream,
                MAX_PAYLOAD_BYTES,
            ),
        ] {
            if value < minimum {
                return Err(ConfigError::LimitTooSmall { limit, minimum });
            }
        }
        Ok(Self {
            limits,
            handshake_timeout,
        })
    }

    pub fn limits(&self) -> &ResourceLimits {
        &self.limits
    }

    pub fn handshake_timeout(&self) -> Duration {
        self.handshake_timeout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimum_limits() -> ResourceLimits {
        ResourceLimits {
            connections: 1,
            streams_per_connection: 1,
            outbound_bytes: MAX_FRAME_BYTES,
            buffered_bytes_per_stream: MAX_PAYLOAD_BYTES,
        }
    }

    #[test]
    fn minimum_policy_preserves_full_size_frame_support() {
        let limits = minimum_limits();
        let config = TransportConfig::new(limits, Duration::from_secs(1)).unwrap();
        assert_eq!(*config.limits(), limits);
        assert_eq!(config.handshake_timeout(), Duration::from_secs(1));
    }

    #[test]
    fn zero_handshake_timeout_is_rejected() {
        assert_eq!(
            TransportConfig::new(minimum_limits(), Duration::ZERO),
            Err(ConfigError::ZeroHandshakeTimeout)
        );
    }

    #[test]
    fn invalid_budgets_identify_the_field_and_required_minimum() {
        for (limit, minimum) in [
            (Limit::Connections, 1),
            (Limit::StreamsPerConnection, 1),
            (Limit::OutboundBytes, MAX_FRAME_BYTES),
            (Limit::BufferedBytesPerStream, MAX_PAYLOAD_BYTES),
        ] {
            for value in [0, minimum - 1] {
                let mut limits = minimum_limits();
                match limit {
                    Limit::Connections => limits.connections = value,
                    Limit::StreamsPerConnection => limits.streams_per_connection = value,
                    Limit::OutboundBytes => limits.outbound_bytes = value,
                    Limit::BufferedBytesPerStream => limits.buffered_bytes_per_stream = value,
                }
                assert_eq!(
                    TransportConfig::new(limits, Duration::from_secs(1)),
                    Err(ConfigError::LimitTooSmall { limit, minimum })
                );
            }
        }
    }
}
