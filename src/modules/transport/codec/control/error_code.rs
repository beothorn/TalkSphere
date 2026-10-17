//! Version 1 wire codes shared by ERROR, CANCEL and GOAWAY.
//! Application failures remain in their own protocol rather than this registry.

use super::PayloadError;

/// Exact wire values; these do not determine connection versus stream scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ErrorCode {
    /// Graceful GOAWAY only.
    NoError = 0x0000,
    /// Malformed or forbidden protocol behavior.
    ProtocolError = 0x0001,
    /// Identity or handshake verification failed.
    AuthenticationFailed = 0x0002,
    /// The frame payload exceeds the wire limit.
    FrameSizeError = 0x0003,
    /// The OPEN protocol ID/version is unsupported.
    UnsupportedProtocol = 0x0004,
    /// Invalid transition on an existing stream.
    StreamStateError = 0x0005,
    /// The stream was not accepted.
    RefusedStream = 0x0006,
    /// The implementation failed while handling the connection.
    InternalError = 0x0007,
    /// No local stream identifiers remain.
    IdExhausted = 0x0008,
    /// The operation was intentionally cancelled.
    Cancelled = 0x0009,
    /// A bounded local resource limit was reached.
    ResourceLimit = 0x000a,
}

impl TryFrom<u16> for ErrorCode {
    type Error = PayloadError;

    /// Look up a numeric code without treating an unknown value as a default.
    ///
    /// # Errors
    /// Returns `UnknownCode` for any unregistered version 1 value.
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        tracing::trace!(
            function = "ErrorCode::try_from",
            "Looking up a control error code"
        );
        match value {
            0 => Ok(Self::NoError),
            1 => Ok(Self::ProtocolError),
            2 => Ok(Self::AuthenticationFailed),
            3 => Ok(Self::FrameSizeError),
            4 => Ok(Self::UnsupportedProtocol),
            5 => Ok(Self::StreamStateError),
            6 => Ok(Self::RefusedStream),
            7 => Ok(Self::InternalError),
            8 => Ok(Self::IdExhausted),
            9 => Ok(Self::Cancelled),
            10 => Ok(Self::ResourceLimit),
            _ => Err(PayloadError::UnknownCode(value)),
        }
    }
}
