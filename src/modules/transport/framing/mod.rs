//! Bounded complete-frame I/O for the future connection driver.
//! One reader and writer belong to one socket; tag state is supplied per direction.
//! Frames returned here are unverified and must not reach application consumers.

mod reader;
mod writer;
use super::codec::{FrameHeader, HeaderError};
pub use reader::FrameReader;
pub use writer::FrameWriter;

/// Framing failures, kept distinct from wire error codes and stream outcomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramingError {
    /// Invalid header; no payload was read or allocated.
    Header(HeaderError),
    /// EOF inside a header, payload or tag.
    Truncated,
    /// Socket failure, including WriteZero or a broken adapter's byte count.
    Io(std::io::ErrorKind),
    /// The reader/writer already failed and cannot be reused.
    Closed,
    /// Integrity mode changed while a frame was in progress.
    PhaseChanged,
    /// Local payload or tag does not match the requested framing shape.
    Shape,
    /// The writer still owns an unfinished frame.
    Busy,
}

/// Complete but unverified bytes. Header and payload lengths are consistent.
pub struct WireFrame {
    header: FrameHeader,
    payload: Vec<u8>,
    tag: Option<[u8; 16]>,
}

impl WireFrame {
    /// Validate local payload length; tag presence is checked against the writer's phase.
    /// # Errors
    /// Returns Shape if the payload does not match the header's length.
    pub fn new(
        header: FrameHeader,
        payload: Vec<u8>,
        tag: Option<[u8; 16]>,
    ) -> Result<Self, FramingError> {
        tracing::trace!(function = "WireFrame::new", "Checking frame shape");
        if payload.len() != header.payload_len() as usize {
            return Err(FramingError::Shape);
        }
        Ok(Self {
            header,
            payload,
            tag,
        })
    }
    /// Copy the validated header without implying authentication.
    pub fn header(&self) -> FrameHeader {
        tracing::trace!(function = "WireFrame::header", "Reading frame metadata");
        self.header
    }
    /// Borrow unverified payload bytes for the integrity/handshake layer.
    pub fn payload(&self) -> &[u8] {
        tracing::trace!(
            function = "WireFrame::payload",
            "Borrowing unverified frame bytes"
        );
        &self.payload
    }
    /// Borrow the received tag, if this direction is tagged.
    pub fn tag(&self) -> Option<&[u8; 16]> {
        tracing::trace!(function = "WireFrame::tag", "Inspecting tag presence");
        self.tag.as_ref()
    }
    /// Transfer payload ownership after the caller has completed validation.
    pub fn into_payload(self) -> Vec<u8> {
        tracing::trace!(
            function = "WireFrame::into_payload",
            "Transferring a frame payload"
        );
        self.payload
    }
}

#[cfg(test)]
mod tests;
