//! Transport contracts and local boundary validation.
//! No driver, socket adapters, wire codecs or authentication are implemented.
//!
//! The upstream contract is at <https://github.com/beothorn/CR2SE/tree/main/modules/transport>.

pub mod api;
pub mod config;
mod error;
pub mod handles;
pub mod ports;

pub use error::TransportError;

/// Attempt to start Transport.
///
/// # Errors
///
/// Always returns [`TransportError::NotImplemented`] until networking is implemented.
/// This function performs no network I/O and creates no runtime state.
pub fn start() -> Result<(), TransportError> {
    Err(TransportError::NotImplemented)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_cannot_report_a_running_transport() {
        assert_eq!(start(), Err(TransportError::NotImplemented));
    }
}
