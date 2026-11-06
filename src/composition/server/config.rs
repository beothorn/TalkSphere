//! Effect-free server policy validation before private storage or network acquisition.
use crate::modules::{
    identity::{api::Contact, contact},
    transport::config::{MAX_FRAME_BYTES, MAX_PAYLOAD_BYTES, ResourceLimits, TransportConfig},
};
use std::{
    collections::BTreeSet,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    time::Duration,
};

/// Unvalidated trusted local input; never construct these options from peer payloads.
pub struct Options {
    /// Explicit protected identity directory; initialization is a separate operation.
    pub identity_store: PathBuf,
    /// Explicit bind address; port zero requests an OS-assigned port.
    pub listen: SocketAddr,
    /// Explicit startup contacts; ID-only contacts cannot be resolved without Discovery.
    pub contacts: Vec<Contact>,
    /// Maximum startup contact count, independent of simultaneous connection capacity.
    pub maximum_contacts: usize,
    /// Transport policy validated by its owning module.
    pub transport: TransportConfig,
    /// Maximum duration allowed for graceful shutdown.
    pub shutdown_timeout: Duration,
}
impl Options {
    /// Supply finite local defaults with an explicit identity path and loopback ephemeral listener.
    /// # Errors
    /// Propagates Transport validation errors if default policy becomes inconsistent.
    pub fn local(
        identity_store: PathBuf,
    ) -> Result<Self, crate::modules::transport::config::ConfigError> {
        tracing::trace!("Constructing finite local server defaults");
        Ok(Self {
            identity_store,
            listen: SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
            contacts: Vec::new(),
            maximum_contacts: 64,
            transport: TransportConfig::new(
                ResourceLimits {
                    connections: 64,
                    streams_per_connection: 128,
                    outbound_bytes: 64 * MAX_FRAME_BYTES,
                    buffered_bytes_per_stream: MAX_PAYLOAD_BYTES,
                },
                Duration::from_secs(10),
            )?,
            shutdown_timeout: Duration::from_secs(5),
        })
    }
}
/// Invalid local policy, without sensitive path or contact diagnostic contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    /// A protected directory must be selected explicitly.
    MissingIdentityStore,
    /// Shutdown must have a finite positive deadline.
    ZeroShutdownTimeout,
    /// Contact capacity must be positive and not exceeded.
    ContactLimit,
    /// A configured monotonic deadline cannot be represented by this platform.
    DeadlineRange,
    /// A configured contact has no usable IP/port.
    InvalidContact,
    /// Repeated binary identities would make startup attempts ambiguous.
    DuplicateContact,
}
/// Immutable validated runtime policy; construction performs no filesystem or socket operations.
pub struct Config(Options);
impl Config {
    /// Validate application-only constraints before effects; Transport policy is already validated.
    /// # Errors
    /// Rejects empty paths, zero shutdown deadlines, contact excess, duplicate IDs and invalid endpoints.
    pub fn new(options: Options) -> Result<Self, ConfigError> {
        tracing::trace!("Validating server policy before runtime effects");
        if options.identity_store.as_os_str().is_empty() {
            return Err(ConfigError::MissingIdentityStore);
        }
        if options.shutdown_timeout.is_zero() {
            return Err(ConfigError::ZeroShutdownTimeout);
        }
        let now = std::time::Instant::now();
        if now.checked_add(options.shutdown_timeout).is_none()
            || now
                .checked_add(options.transport.handshake_timeout())
                .is_none()
        {
            return Err(ConfigError::DeadlineRange);
        }
        if options.maximum_contacts == 0 || options.contacts.len() > options.maximum_contacts {
            return Err(ConfigError::ContactLimit);
        }
        let mut ids = BTreeSet::new();
        for entry in &options.contacts {
            if entry.endpoint.is_none() || contact::format(*entry).is_err() {
                return Err(ConfigError::InvalidContact);
            }
            if !ids.insert(entry.expected_id) {
                return Err(ConfigError::DuplicateContact);
            }
        }
        Ok(Self(options))
    }
    /// Borrow immutable validated policy for local runtime composition.
    pub fn options(&self) -> &Options {
        tracing::trace!("Reading validated server configuration");
        &self.0
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::identity::api::IdentityId;
    // Default construction preserves full-size frames and loopback port-zero support without I/O.
    #[test]
    fn local_defaults_are_finite_and_valid() {
        let config =
            Config::new(Options::local("not-created-by-validation".into()).unwrap()).unwrap();
        let options = config.options();
        assert!(options.listen.ip().is_loopback());
        assert_eq!(options.listen.port(), 0);
        assert_eq!(
            options.transport.handshake_timeout(),
            Duration::from_secs(10)
        );
        assert_eq!(
            options.transport.limits().buffered_bytes_per_stream,
            MAX_PAYLOAD_BYTES
        );
        assert_eq!(options.shutdown_timeout, Duration::from_secs(5));
    }
    // Unrepresentable deadlines fail before key-store or socket acquisition rather than during a later shutdown.
    #[test]
    fn unrepresentable_deadlines_fail_before_effects() {
        let mut options = Options::local("fixture".into()).unwrap();
        options.shutdown_timeout = Duration::MAX;
        assert!(matches!(
            Config::new(options),
            Err(ConfigError::DeadlineRange)
        ));
        let mut options = Options::local("fixture".into()).unwrap();
        options.transport =
            TransportConfig::new(*options.transport.limits(), Duration::MAX).unwrap();
        assert!(matches!(
            Config::new(options),
            Err(ConfigError::DeadlineRange)
        ));
    }
    // Invalid options fail before runtime acquisition; exact contact capacity and distinct IDs remain valid.
    #[test]
    fn configuration_rejects_invalid_and_ambiguous_input() {
        let make = || Options::local("fixture".into()).unwrap();
        let mut options = make();
        options.identity_store = PathBuf::new();
        assert!(matches!(
            Config::new(options),
            Err(ConfigError::MissingIdentityStore)
        ));
        let mut options = make();
        options.shutdown_timeout = Duration::ZERO;
        assert!(matches!(
            Config::new(options),
            Err(ConfigError::ZeroShutdownTimeout)
        ));
        let mut options = make();
        options.maximum_contacts = 0;
        assert!(matches!(
            Config::new(options),
            Err(ConfigError::ContactLimit)
        ));
        let contact = Contact {
            expected_id: IdentityId([1; 32]),
            endpoint: Some("127.0.0.1:1".parse().unwrap()),
        };
        let mut options = make();
        options.maximum_contacts = 1;
        options.contacts = vec![contact];
        assert!(Config::new(options).is_ok());
        let mut options = make();
        options.maximum_contacts = 1;
        options.contacts = vec![contact, contact];
        assert!(matches!(
            Config::new(options),
            Err(ConfigError::ContactLimit)
        ));
        let mut options = make();
        options.contacts = vec![contact, contact];
        assert!(matches!(
            Config::new(options),
            Err(ConfigError::DuplicateContact)
        ));
        for endpoint in [None, Some("127.0.0.1:0".parse().unwrap())] {
            let mut options = make();
            options.contacts = vec![Contact {
                endpoint,
                ..contact
            }];
            assert!(matches!(
                Config::new(options),
                Err(ConfigError::InvalidContact)
            ));
        }
    }
}
