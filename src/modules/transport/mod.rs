//! Transport framing, authentication orchestration, multiplexed streams and TCP control.
//! Production primitives are injected through explicit Identity/Cryptography contracts.
//! Configured authenticated CLI composition is provided by composition::server.
//!
//! Deterministic dependencies are available only to internal unit tests.
//! ```compile_fail
//! use talksphere::modules::transport::test_support::socket::ScriptedSocket;
//! ```
//!
//! The upstream contract is at <https://github.com/beothorn/CR2SE/tree/main/modules/transport>.

pub mod adapters;
pub mod api;
pub mod codec;
pub mod config;
pub mod connection;
pub mod driver;
mod error;
pub mod failure;
pub mod framing;
pub mod handles;
pub mod handshake;
pub mod integrity;
pub mod ports;
pub mod resources;
pub mod stream;

#[cfg(test)]
pub(crate) mod test_support;

pub use error::TransportError;

/// Reject argument-free startup, which cannot select protected credentials or local policy.
///
/// # Errors
///
/// Always returns [`TransportError::RuntimeUnavailable`]; use the configured server composition or serve command.
/// This function performs no network I/O and creates no runtime state.
pub fn start() -> Result<(), TransportError> {
    tracing::trace!(
        function = "transport::start",
        "Checking normal server composition availability"
    );
    Err(TransportError::RuntimeUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Argument-free startup cannot guess protected configuration or report an authenticated server.
    #[test]
    fn missing_runtime_cannot_report_a_running_server() {
        assert_eq!(start(), Err(TransportError::RuntimeUnavailable));
    }
}
