//! Structured payload rejection reasons returned to the Transport driver.
//! Errors contain lengths/codes only, never the peer's diagnostic text.

use std::{error::Error, fmt};

/// Malformed control payloads; the future driver decides how to close safely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadError {
    /// A fixed prefix or token is incomplete.
    TooShort { minimum: usize, actual: usize },
    /// The payload has fewer/more bytes than its exact declared shape.
    LengthMismatch { expected: usize, actual: usize },
    /// The diagnostic exceeds the protocol's byte limit.
    ReasonTooLong { actual: usize },
    /// The reason cannot be decoded as UTF-8 without loss.
    InvalidUtf8,
    /// Text fields must not begin with a UTF-8 byte-order mark.
    ByteOrderMark,
    /// The version 1 code registry has no entry for this value.
    UnknownCode(u16),
    /// ERROR and CANCEL cannot carry the graceful GOAWAY code.
    NoErrorForbidden,
}

impl fmt::Display for PayloadError {
    /// Render a safe diagnostic without exposing peer-controlled text.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { minimum, actual } => {
                write!(f, "payload needs at least {minimum} bytes, got {actual}")
            }
            Self::LengthMismatch { expected, actual } => {
                write!(f, "payload needs exactly {expected} bytes, got {actual}")
            }
            Self::ReasonTooLong { actual } => {
                write!(f, "diagnostic has {actual} bytes; maximum is 1024")
            }
            Self::InvalidUtf8 => f.write_str("diagnostic is not valid UTF-8"),
            Self::ByteOrderMark => f.write_str("diagnostic begins with a byte-order mark"),
            Self::UnknownCode(code) => write!(f, "unknown control error code: {code:#06x}"),
            Self::NoErrorForbidden => f.write_str("NO_ERROR is only permitted in GOAWAY"),
        }
    }
}

impl Error for PayloadError {}
