//! Connection-bound frame integrity and independent implicit sequence counters.
//! The driver supplies established keys and closes without a response on failure.
use super::{
    codec::FrameHeader,
    framing::{FramingError, WireFrame},
    handshake::{DirectionKeys, SessionKeys, nonce},
    ports::{DependencyError, crypto::Cryptography},
};

/// Integrity failures never authorize delivery or an unauthenticated error reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrityError {
    /// Session secrets have already been released.
    Closed,
    /// A sealed outbound frame has not yet completed its write.
    Busy,
    /// Missing/extra tag or invalid local completion ordering.
    Shape,
    /// No sequence number may wrap or be reused.
    Exhausted,
    /// Crypto rejected a tag or could not perform the operation.
    Dependency(DependencyError),
    /// Local frame shape was inconsistent.
    Framing(FramingError),
}

/// A frame released only after its expected directional tag verifies.
pub struct VerifiedFrame(WireFrame);
impl VerifiedFrame {
    /// Return metadata whose exact canonical wire bytes were authenticated.
    pub fn header(&self) -> FrameHeader {
        tracing::trace!(
            function = "VerifiedFrame::header",
            "Reading verified frame metadata"
        );
        self.0.header()
    }
    /// Borrow verified bytes; payload semantics and stream state still require checks.
    pub fn payload(&self) -> &[u8] {
        tracing::trace!(
            function = "VerifiedFrame::payload",
            "Borrowing verified payload bytes"
        );
        self.0.payload()
    }
    /// Transfer verified bytes to the state machine without copying.
    pub fn into_payload(self) -> Vec<u8> {
        tracing::trace!(
            function = "VerifiedFrame::into_payload",
            "Transferring verified payload ownership"
        );
        self.0.into_payload()
    }
}

/// Per-connection keys; only one sealed outbound frame may be pending at a time.
pub struct Integrity<K> {
    send: Option<DirectionKeys<K>>,
    receive: Option<DirectionKeys<K>>,
    send_sequence: u64,
    receive_sequence: u64,
    pending_send: bool,
}
impl<K> Integrity<K> {
    /// Consume fresh, locally oriented session keys and start both sequences at zero.
    pub fn new(keys: SessionKeys<K>) -> Self {
        tracing::trace!(
            function = "Integrity::new",
            "Starting independent connection sequences"
        );
        Self {
            send: Some(keys.send),
            receive: Some(keys.receive),
            send_sequence: 0,
            receive_sequence: 0,
            pending_send: false,
        }
    }
    /// Seal one frame, reserving its sequence until sent() reports a complete write.
    /// # Errors
    /// Busy rejects another pending frame without changing state. Other errors
    /// release secrets; never retry/discard a sealed frame while keeping this session.
    pub fn seal<C: Cryptography<IntegrityKey = K>>(
        &mut self,
        frame: WireFrame,
        crypto: &C,
    ) -> Result<WireFrame, IntegrityError> {
        tracing::trace!(
            function = "Integrity::seal",
            "Authenticating a serialized outbound frame"
        );
        if self.send.is_none() {
            return Err(IntegrityError::Closed);
        }
        if self.pending_send {
            return Err(IntegrityError::Busy);
        }
        if frame.tag().is_some() {
            return self.fail(IntegrityError::Shape);
        }
        if self.send_sequence == u64::MAX {
            return self.fail(IntegrityError::Exhausted);
        }
        let direction = self.send.as_ref().ok_or(IntegrityError::Closed)?;
        let aad = [frame.header().encode().as_slice(), frame.payload()].concat();
        let tag = match crypto.tag(
            &direction.key,
            &nonce(&direction.nonce_prefix, self.send_sequence),
            &aad,
        ) {
            Ok(tag) => tag,
            Err(error) => return self.fail(IntegrityError::Dependency(error)),
        };
        let reservation = frame
            .reservation
            .as_ref()
            .map(super::resources::Reservation::retain);
        let header = frame.header();
        let mut frame = WireFrame::new(header, frame.into_payload(), Some(tag))
            .map_err(IntegrityError::Framing)?;
        frame.reservation = reservation;
        self.pending_send = true;
        Ok(frame)
    }
    /// Advance the reserved send sequence only after the entire sealed frame is written.
    /// # Errors
    /// A completion without a pending frame or on a closed session is invalid.
    pub fn sent(&mut self) -> Result<(), IntegrityError> {
        tracing::trace!(
            function = "Integrity::sent",
            "Committing a completed outbound sequence"
        );
        if self.send.is_none() {
            return Err(IntegrityError::Closed);
        }
        if !self.pending_send {
            return self.fail(IntegrityError::Shape);
        }
        self.send_sequence = self
            .send_sequence
            .checked_add(1)
            .ok_or(IntegrityError::Exhausted)?;
        self.pending_send = false;
        Ok(())
    }
    /// Verify exactly the next inbound sequence, advancing only on complete success.
    /// # Errors
    /// Missing/invalid tags, dependency errors and exhaustion release all keys.
    /// No error response is safe after tag failure; the owner must close its socket.
    pub fn verify<C: Cryptography<IntegrityKey = K>>(
        &mut self,
        frame: WireFrame,
        crypto: &C,
    ) -> Result<VerifiedFrame, IntegrityError> {
        tracing::trace!(
            function = "Integrity::verify",
            "Checking the expected inbound sequence"
        );
        let Some(direction) = self.receive.as_ref() else {
            return Err(IntegrityError::Closed);
        };
        let Some(tag) = frame.tag() else {
            return self.fail(IntegrityError::Shape);
        };
        let aad = [frame.header().encode().as_slice(), frame.payload()].concat();
        if let Err(error) = crypto.verify_tag(
            &direction.key,
            &nonce(&direction.nonce_prefix, self.receive_sequence),
            &aad,
            tag,
        ) {
            return self.fail(IntegrityError::Dependency(error));
        }
        let Some(next) = self.receive_sequence.checked_add(1) else {
            return self.fail(IntegrityError::Exhausted);
        };
        self.receive_sequence = next;
        Ok(VerifiedFrame(frame))
    }
    /// Release both keys, including a reserved outbound sequence, without reuse.
    pub fn close(&mut self) {
        tracing::trace!(
            function = "Integrity::close",
            "Releasing connection integrity keys"
        );
        self.send = None;
        self.receive = None;
        self.pending_send = false;
    }
    /// Report whether the session's secrets have been released.
    pub fn is_closed(&self) -> bool {
        tracing::trace!(
            function = "Integrity::is_closed",
            "Checking integrity session lifetime"
        );
        self.send.is_none()
    }
    // All fatal paths share key release, so no failed verification can be retried.
    fn fail<T>(&mut self, error: IntegrityError) -> Result<T, IntegrityError> {
        tracing::trace!(
            function = "Integrity::fail",
            "Invalidating a failed integrity session"
        );
        self.close();
        Err(error)
    }
}

#[cfg(test)]
mod tests;
