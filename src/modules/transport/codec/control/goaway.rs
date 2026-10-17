//! GOAWAY's wire payload, separate from connection draining and ID state checks.
//! The shared diagnostic codec supplies text bounds and the error registry.

use super::{ErrorCode, PayloadError, diagnostic::Diagnostic};

/// Validated GOAWAY payload shape, borrowing its diagnostic reason.
/// The driver must still check ID parity, the opened-ID bound and repeated limits.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct GoAway<'a> {
    last_accepted_remote_stream_id: u32,
    diagnostic: Diagnostic<'a>,
}

impl<'a> GoAway<'a> {
    /// Retain the last accepted remote stream ID and a code, including NO_ERROR.
    /// `reason` is borrowed UTF-8 diagnostic text and must contain no secrets.
    ///
    /// # Errors
    /// Rejects reasons above 1,024 bytes and a leading byte-order mark.
    pub fn new(
        last_accepted_remote_stream_id: u32,
        code: ErrorCode,
        reason: &'a str,
    ) -> Result<Self, PayloadError> {
        tracing::trace!(
            function = "GoAway::new",
            "Validating an outgoing shutdown payload"
        );
        Ok(Self {
            last_accepted_remote_stream_id,
            diagnostic: Diagnostic::new(code, reason, true)?,
        })
    }

    /// Decode the big-endian ID and validate the complete diagnostic envelope.
    ///
    /// # Errors
    /// Rejects short/trailing input, excessive or invalid text and unknown codes.
    /// The result is not authenticated and does not initiate connection draining.
    pub fn decode(payload: &'a [u8]) -> Result<Self, PayloadError> {
        tracing::trace!(function = "GoAway::decode", "Decoding a shutdown payload");
        if payload.len() < 8 {
            return Err(PayloadError::TooShort {
                minimum: 8,
                actual: payload.len(),
            });
        }
        let last_accepted_remote_stream_id =
            u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
        let diagnostic = Diagnostic::decode(&payload[4..], true).map_err(|error| match error {
            // Report full GOAWAY sizes rather than sizes of its nested envelope.
            PayloadError::LengthMismatch { expected, actual } => PayloadError::LengthMismatch {
                expected: expected + 4,
                actual: actual + 4,
            },
            other => other,
        })?;
        Ok(Self {
            last_accepted_remote_stream_id,
            diagnostic,
        })
    }

    /// Serialize the retained ID and diagnostic into a buffer of at most 1,032 bytes.
    pub fn encode(self) -> Vec<u8> {
        tracing::trace!(function = "GoAway::encode", "Encoding a shutdown payload");
        let mut bytes = Vec::with_capacity(8 + self.diagnostic.reason.len());
        bytes.extend_from_slice(&self.last_accepted_remote_stream_id.to_be_bytes());
        self.diagnostic.append_to(&mut bytes);
        bytes
    }

    /// Return the wire ID unchanged for later connection-state validation.
    pub fn last_accepted_remote_stream_id(self) -> u32 {
        tracing::trace!(
            function = "GoAway::last_accepted_remote_stream_id",
            "Reading the shutdown stream limit"
        );
        self.last_accepted_remote_stream_id
    }

    /// Return the shutdown code; NO_ERROR represents graceful draining.
    pub fn code(self) -> ErrorCode {
        tracing::trace!(function = "GoAway::code", "Reading the shutdown code");
        self.diagnostic.code
    }

    /// Borrow the untrusted diagnostic without using it to decide behavior.
    pub fn reason(self) -> &'a str {
        tracing::trace!(
            function = "GoAway::reason",
            "Borrowing shutdown diagnostic text"
        );
        self.diagnostic.reason
    }
}
