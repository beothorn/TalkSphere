//! Application server lifecycle contracts; protocol state remains owned by Transport.
pub mod config;

/// Observable runtime stages; a bound listener alone is insufficient to report readiness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Configuration is validated and no effects have started.
    Configured,
    /// Credentials and dependencies are being acquired.
    Starting,
    /// All startup prerequisites and listener binding have succeeded.
    Ready,
    /// New work is refused while accepted work drains to a deadline.
    Stopping,
    /// All owned runtime resources have been released.
    Stopped,
}

/// Secret-free lifecycle failure categories for application boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// The selected platform lacks a protected storage or runtime adapter.
    Unsupported,
    /// Credentials are absent, inaccessible or inconsistent.
    Identity,
    /// Listener binding or outgoing connection setup failed.
    Network,
    /// Readiness worker or runtime event processing failed.
    Runtime,
    /// Controlled draining exceeded its configured deadline.
    ShutdownTimeout,
}

#[cfg(test)]
mod test_support;

#[cfg(target_os = "linux")]
pub mod credentials;

#[cfg(target_os = "linux")]
pub mod transport;

pub mod pump;

#[cfg(target_os = "linux")]
pub mod contacts;

pub mod routing;

#[cfg(target_os = "linux")]
pub mod shutdown;

#[cfg(target_os = "linux")]
pub mod run;

#[cfg(all(test, target_os = "linux"))]
#[path = "tests/real_wire.rs"]
mod real_wire;
