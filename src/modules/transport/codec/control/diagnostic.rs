//! Shared bounded diagnostics for ERROR/CANCEL and GOAWAY.
//! Decoders borrow validated text from the input, avoiding payload allocations.

use super::{ErrorCode, MAX_REASON_BYTES, PayloadError};

/// Validated ERROR or CANCEL payload; the enclosing frame determines its scope.
/// Diagnostic text is untrusted information, never a machine-readable decision.
/// Callers must supply secret-free reasons and sanitize text before displaying it.
///
/// ```
/// use talksphere::modules::transport::codec::control::{ErrorCode, ErrorPayload};
/// let payload = ErrorPayload::new(ErrorCode::Cancelled, "done")?;
/// assert_eq!(payload.encode(), [0, 9, 0, 4, b'd', b'o', b'n', b'e']);
/// # Ok::<(), talksphere::modules::transport::codec::control::PayloadError>(())
/// ```
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ErrorPayload<'a>(Diagnostic<'a>);

impl<'a> ErrorPayload<'a> {
    /// Borrow an outgoing reason, checking its byte bound and forbidding NO_ERROR.
    ///
    /// # Errors
    /// Rejects NO_ERROR, reasons above 1,024 UTF-8 bytes and a leading BOM.
    pub fn new(code: ErrorCode, reason: &'a str) -> Result<Self, PayloadError> {
        tracing::trace!(
            function = "ErrorPayload::new",
            "Validating an outgoing error or cancellation"
        );
        Diagnostic::new(code, reason, false).map(Self)
    }

    /// Validate the entire ERROR/CANCEL payload and borrow its diagnostic text.
    ///
    /// # Errors
    /// Rejects truncated/trailing bytes, oversized or invalid text, unknown codes
    /// and NO_ERROR. Successful parsing does not authenticate the payload.
    pub fn decode(payload: &'a [u8]) -> Result<Self, PayloadError> {
        tracing::trace!(
            function = "ErrorPayload::decode",
            "Decoding an error or cancellation payload"
        );
        Diagnostic::decode(payload, false).map(Self)
    }

    /// Serialize the validated code, byte length and reason into a bounded buffer.
    pub fn encode(self) -> Vec<u8> {
        tracing::trace!(
            function = "ErrorPayload::encode",
            "Encoding an error or cancellation payload"
        );
        let mut bytes = Vec::with_capacity(4 + self.0.reason.len());
        self.0.append_to(&mut bytes);
        bytes
    }

    /// Return the registered code used for machine-readable decisions.
    pub fn code(self) -> ErrorCode {
        tracing::trace!(function = "ErrorPayload::code", "Reading the error code");
        self.0.code
    }

    /// Borrow diagnostic text without interpreting, logging or sanitizing it.
    pub fn reason(self) -> &'a str {
        tracing::trace!(
            function = "ErrorPayload::reason",
            "Borrowing untrusted diagnostic text"
        );
        self.0.reason
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Diagnostic<'a> {
    pub(super) code: ErrorCode,
    pub(super) reason: &'a str,
}

impl<'a> Diagnostic<'a> {
    // Keep the wire text rules in one place; only GOAWAY may allow NO_ERROR.
    pub(super) fn new(
        code: ErrorCode,
        reason: &'a str,
        allow_no_error: bool,
    ) -> Result<Self, PayloadError> {
        tracing::trace!(
            function = "Diagnostic::new",
            "Checking diagnostic text and code constraints"
        );
        if reason.len() > MAX_REASON_BYTES {
            return Err(PayloadError::ReasonTooLong {
                actual: reason.len(),
            });
        }
        if reason.starts_with('\u{feff}') {
            return Err(PayloadError::ByteOrderMark);
        }
        if code == ErrorCode::NoError && !allow_no_error {
            return Err(PayloadError::NoErrorForbidden);
        }
        Ok(Self { code, reason })
    }

    // Reject length mismatches before decoding text, so no trailing data is hidden.
    pub(super) fn decode(payload: &'a [u8], allow_no_error: bool) -> Result<Self, PayloadError> {
        tracing::trace!(
            function = "Diagnostic::decode",
            "Checking the diagnostic envelope before borrowing text"
        );
        if payload.len() < 4 {
            return Err(PayloadError::TooShort {
                minimum: 4,
                actual: payload.len(),
            });
        }
        let code = u16::from_be_bytes([payload[0], payload[1]]);
        let reason_len = usize::from(u16::from_be_bytes([payload[2], payload[3]]));
        if reason_len > MAX_REASON_BYTES {
            return Err(PayloadError::ReasonTooLong { actual: reason_len });
        }
        // The fixed 1,024-byte bound makes this addition safe on supported targets.
        let expected = 4 + reason_len;
        if payload.len() != expected {
            return Err(PayloadError::LengthMismatch {
                expected,
                actual: payload.len(),
            });
        }
        let reason = std::str::from_utf8(&payload[4..]).map_err(|_| PayloadError::InvalidUtf8)?;
        Self::new(ErrorCode::try_from(code)?, reason, allow_no_error)
    }

    // Validation caps the length at 1,024, so its conversion to uint16 is exact.
    pub(super) fn append_to(self, bytes: &mut Vec<u8>) {
        tracing::trace!(
            function = "Diagnostic::append_to",
            "Appending a validated diagnostic envelope"
        );
        bytes.extend_from_slice(&(self.code as u16).to_be_bytes());
        bytes.extend_from_slice(&(self.reason.len() as u16).to_be_bytes());
        bytes.extend_from_slice(self.reason.as_bytes());
    }
}
