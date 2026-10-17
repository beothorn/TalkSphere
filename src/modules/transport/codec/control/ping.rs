//! The opaque eight-byte token shared by PING and PONG.
//! Connection code handles matching and timeouts; this codec preserves bytes.

use super::PayloadError;

/// An arbitrary token whose exact bytes must be echoed in the PONG payload.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PingToken([u8; 8]);

impl PingToken {
    /// Retain a caller-selected token without interpreting its contents.
    pub fn new(bytes: [u8; 8]) -> Self {
        tracing::trace!(
            function = "PingToken::new",
            "Retaining an opaque ping token"
        );
        Self(bytes)
    }

    /// Read an entire PING or PONG payload without allocating or accepting extras.
    ///
    /// # Errors
    /// Returns `LengthMismatch` unless the payload is exactly eight bytes.
    pub fn decode(payload: &[u8]) -> Result<Self, PayloadError> {
        tracing::trace!(
            function = "PingToken::decode",
            "Checking a ping or pong payload length"
        );
        payload
            .try_into()
            .map(Self)
            .map_err(|_| PayloadError::LengthMismatch {
                expected: 8,
                actual: payload.len(),
            })
    }

    /// Return the unchanged bytes for either a PING or its PONG response.
    pub fn encode(self) -> [u8; 8] {
        tracing::trace!(
            function = "PingToken::encode",
            "Encoding an opaque ping token"
        );
        self.0
    }
}
