//! Version 1 wire codecs, starting with preliminary header validation.
//!
//! Baseline: <https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/README.md#3-frame-format>.
//! A decoded header is untrusted: payload shape, integrity and connection/stream
//! state must still be checked before dispatch. This module performs no I/O.

mod error;
mod frame_type;
mod header;

pub use error::HeaderError;
pub use frame_type::FrameType;
pub use header::{FrameHeader, FrameProtection};

pub const VERSION: u8 = 1;
pub const HEADER_BYTES: usize = 12;
pub const INTEGRITY_TAG_BYTES: usize = 16;
/// The fixed wire limit, independent of local resource policy.
pub const MAX_PAYLOAD_BYTES: usize = 65_536;

#[cfg(test)]
mod tests;
