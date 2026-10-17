//! Single-frame writer preserving ordering over short writes and Pending.
//! Queue policy belongs to the driver; this object retains at most one frame.
use super::{FramingError, WireFrame};
use crate::modules::transport::{codec::FrameProtection, ports::io::Socket};
use std::{
    io::ErrorKind,
    task::{Context, Poll},
};

/// Owns an entire serialized frame until its last byte is accepted by the socket.
#[derive(Default)]
pub struct FrameWriter {
    bytes: Vec<u8>,
    offset: usize,
    failed: bool,
}

impl FrameWriter {
    /// Admit exactly one frame without writing any bytes.
    /// # Errors
    /// Busy preserves the current frame; Shape rejects inconsistent tag phase;
    /// Closed means this writer has failed and must not be reused.
    pub fn start(&mut self, frame: WireFrame, phase: FrameProtection) -> Result<(), FramingError> {
        tracing::trace!(
            function = "FrameWriter::start",
            "Admitting one complete frame"
        );
        if self.failed {
            return Err(FramingError::Closed);
        }
        if !self.bytes.is_empty() {
            return Err(FramingError::Busy);
        }
        if frame.tag.is_some() != (phase == FrameProtection::Tagged) {
            return Err(FramingError::Shape);
        }
        let length = frame
            .header
            .frame_len(phase)
            .map_err(FramingError::Header)?;
        self.bytes = Vec::with_capacity(length);
        self.bytes.extend_from_slice(&frame.header.encode());
        self.bytes.extend_from_slice(&frame.payload);
        if let Some(tag) = frame.tag {
            self.bytes.extend_from_slice(&tag);
        }
        self.offset = 0;
        Ok(())
    }
    /// Finish the current frame before another can be admitted; an idle writer is ready.
    /// # Errors
    /// Zero progress or socket failure permanently disables the writer.
    pub fn poll_flush(
        &mut self,
        socket: &mut dyn Socket,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), FramingError>> {
        tracing::trace!(
            function = "FrameWriter::poll_flush",
            "Advancing a serialized frame write"
        );
        if self.failed {
            return Poll::Ready(Err(FramingError::Closed));
        }
        for _ in 0..64 {
            if self.offset == self.bytes.len() {
                self.bytes.clear();
                self.offset = 0;
                return Poll::Ready(Ok(()));
            }
            let remaining = self.bytes.len() - self.offset;
            let error = match socket.poll_write(cx, &self.bytes[self.offset..]) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(error)) if error.kind() == ErrorKind::Interrupted => continue,
                Poll::Ready(Err(error)) => error.kind(),
                Poll::Ready(Ok(0)) => ErrorKind::WriteZero,
                Poll::Ready(Ok(count)) if count <= remaining => {
                    self.offset += count;
                    continue;
                }
                Poll::Ready(Ok(_)) => ErrorKind::InvalidData,
            };
            self.failed = true;
            self.bytes = Vec::new();
            return Poll::Ready(Err(FramingError::Io(error)));
        }
        cx.waker().wake_by_ref();
        Poll::Pending
    }
    /// Report whether a frame still owns the writer, including a partial write.
    pub fn is_busy(&self) -> bool {
        tracing::trace!(
            function = "FrameWriter::is_busy",
            "Checking writer occupancy"
        );
        !self.bytes.is_empty()
    }
}
