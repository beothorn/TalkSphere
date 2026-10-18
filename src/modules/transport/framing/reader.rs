//! Incremental reader; validation precedes bounded body allocation.
//! Errors latch permanently so callers cannot accidentally resynchronize.
use super::{FramingError, WireFrame};
use crate::modules::transport::{
    codec::{FrameHeader, FrameProtection},
    ports::io::Socket,
};
use std::{
    io::ErrorKind,
    task::{Context, Poll},
};

/// One frame at a time, retaining progress across Pending without over-reading.
#[derive(Default)]
pub struct FrameReader {
    bytes: Vec<u8>,
    offset: usize,
    header: Option<FrameHeader>,
    phase: Option<FrameProtection>,
    failed: bool,
    eof: bool,
}

impl FrameReader {
    /// Read one complete unverified frame; None is EOF exactly at a boundary.
    /// # Errors
    /// Rejects malformed/truncated frames, changed phases and socket failures.
    /// A failure permanently disables this reader; the owner must close the socket.
    pub fn poll_frame(
        &mut self,
        socket: &mut dyn Socket,
        cx: &mut Context<'_>,
        phase: FrameProtection,
    ) -> Poll<Result<Option<WireFrame>, FramingError>> {
        tracing::trace!(
            function = "FrameReader::poll_frame",
            "Advancing a bounded frame read"
        );
        if self.failed {
            return Poll::Ready(Err(FramingError::Closed));
        }
        if self.eof {
            return Poll::Ready(Ok(None));
        }
        if self.phase.is_some_and(|old| old != phase) {
            return self.fail(FramingError::PhaseChanged);
        }
        self.phase = Some(phase);
        if self.bytes.is_empty() {
            self.bytes.resize(12, 0);
        }
        // Bound synchronous work even when an adapter delivers one byte per poll.
        for _ in 0..64 {
            if self.offset == self.bytes.len() {
                if let Some(header) = self.header.take() {
                    let mut bytes = std::mem::take(&mut self.bytes);
                    let tag = if phase == FrameProtection::Tagged {
                        let mut tag = [0; 16];
                        tag.copy_from_slice(&bytes[bytes.len() - 16..]);
                        bytes.truncate(bytes.len() - 16);
                        Some(tag)
                    } else {
                        None
                    };
                    bytes.drain(..12);
                    self.offset = 0;
                    self.phase = None;
                    return Poll::Ready(Ok(Some(WireFrame {
                        reservation: None,
                        header,
                        payload: bytes,
                        tag,
                    })));
                }
                let header = match FrameHeader::decode(&self.bytes) {
                    Ok(header) => header,
                    Err(error) => return self.fail(FramingError::Header(error)),
                };
                let length = match header.frame_len(phase) {
                    Ok(length) => length,
                    Err(error) => return self.fail(FramingError::Header(error)),
                };
                self.header = Some(header);
                self.bytes.resize(length, 0);
                continue;
            }
            let remaining = self.bytes.len() - self.offset;
            match socket.poll_read(cx, &mut self.bytes[self.offset..]) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(error)) if error.kind() == ErrorKind::Interrupted => continue,
                Poll::Ready(Err(error)) => return self.fail(FramingError::Io(error.kind())),
                Poll::Ready(Ok(0)) if self.offset == 0 => {
                    self.eof = true;
                    self.bytes.clear();
                    return Poll::Ready(Ok(None));
                }
                Poll::Ready(Ok(0)) => return self.fail(FramingError::Truncated),
                Poll::Ready(Ok(count)) if count <= remaining => self.offset += count,
                Poll::Ready(Ok(_)) => return self.fail(FramingError::Io(ErrorKind::InvalidData)),
            }
        }
        cx.waker().wake_by_ref();
        Poll::Pending
    }
    // Discard all incomplete bytes and prohibit further socket reads after failure.
    fn fail(&mut self, error: FramingError) -> Poll<Result<Option<WireFrame>, FramingError>> {
        tracing::trace!(
            function = "FrameReader::fail",
            "Discarding an invalid incomplete frame"
        );
        self.failed = true;
        self.bytes = Vec::new();
        Poll::Ready(Err(error))
    }
}
