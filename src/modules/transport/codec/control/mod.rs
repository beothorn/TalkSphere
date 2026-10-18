//! Control payload formats used by the Transport connection state.
//! These codecs validate bytes, not authentication or connection/stream state.
//! Shared diagnostics keep ERROR/CANCEL and GOAWAY text rules consistent.

mod diagnostic;
mod error;
mod error_code;
mod goaway;
mod ping;

pub use diagnostic::ErrorPayload;
pub use error::PayloadError;
pub use error_code::ErrorCode;
pub use goaway::GoAway;
pub use ping::PingToken;

/// Maximum diagnostic length in UTF-8 bytes, not Unicode characters.
pub const MAX_REASON_BYTES: usize = 1_024;

#[cfg(test)]
mod tests;
