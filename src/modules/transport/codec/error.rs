use std::{error::Error, fmt};

use super::FrameType;

/// Preliminary header failures, not serialized wire error codes.
///
/// A receiver must terminate on invalid headers without scanning for another
/// boundary. Unsupported versions must never cause a wire error response.
/// Response/closure orchestration belongs to the future connection driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderError {
    InvalidLength {
        actual: usize,
    },
    UnsupportedVersion(u8),
    NonzeroFlags(u16),
    UnknownType(u8),
    PayloadTooLarge(u32),
    InvalidStreamId {
        frame_type: FrameType,
        stream_id: u32,
    },
    SizeOverflow,
}

impl fmt::Display for HeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength { actual } => {
                write!(f, "expected a 12-byte header, got {actual} bytes")
            }
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported frame version: {version:#04x}")
            }
            Self::NonzeroFlags(flags) => write!(f, "nonzero frame flags: {flags:#06x}"),
            Self::UnknownType(value) => write!(f, "unknown frame type: {value:#04x}"),
            Self::PayloadTooLarge(length) => {
                write!(f, "payload length {length} exceeds 65536 bytes")
            }
            Self::InvalidStreamId {
                frame_type,
                stream_id,
            } => write!(f, "invalid stream ID {stream_id} for {frame_type:?}"),
            Self::SizeOverflow => f.write_str("frame size does not fit the platform size type"),
        }
    }
}

impl Error for HeaderError {}
