//! Focused session half-close, terminal outcomes and connection cleanup.
use super::*;

impl Session {
    /// Queue END for the local direction without closing the peer's direction.
    /// # Errors
    /// Rejects foreign authority, inactive streams or a repeated local END.
    pub fn end(&mut self, owner: &StreamOwner, stream: &StreamHandle) -> Result<(), SessionError> {
        tracing::trace!(
            function = "Session::end",
            "Half-closing local stream output"
        );
        if self.closed {
            return Err(SessionError::Closed);
        }
        stream
            .authorize(self.handle.id(), owner)
            .map_err(SessionError::Authority)?;
        let state = self
            .streams
            .get_mut(&stream.wire_id().get())
            .ok_or(SessionError::Protocol)?;
        if !state.local_open {
            return Err(SessionError::Protocol);
        }
        let mut output = frame(FrameType::End, stream.wire_id().get(), vec![])?;
        output.reservation = Some(self.resources.outbound(28).ok_or(SessionError::Capacity)?);
        state.local_open = false;
        self.outbound.push_back(output);
        Ok(())
    }

    /// Cancel both stream directions and discard queued local production.
    /// # Errors
    /// Rejects foreign authority or an already terminal stream without queueing work.
    pub fn cancel(
        &mut self,
        owner: &StreamOwner,
        stream: &StreamHandle,
        reason: CancellationReason,
    ) -> Result<(), SessionError> {
        tracing::trace!(
            function = "Session::cancel",
            "Cancelling both stream directions"
        );
        if self.closed {
            return Err(SessionError::Closed);
        }
        stream
            .authorize(self.handle.id(), owner)
            .map_err(SessionError::Authority)?;
        let id = stream.wire_id().get();
        if !self.streams.contains_key(&id) {
            return Err(SessionError::Protocol);
        }
        let code = match reason {
            CancellationReason::Cancelled => ErrorCode::Cancelled,
            CancellationReason::ResourceLimit => ErrorCode::ResourceLimit,
            CancellationReason::InternalError => ErrorCode::InternalError,
        };
        self.terminate(
            id,
            StreamOutcome::Cancelled {
                origin: FailureOrigin::Local,
                reason,
            },
        );
        let payload = ErrorPayload::new(code, "")
            .map_err(|_| SessionError::Protocol)?
            .encode();
        if self.queue(frame(FrameType::Cancel, id, payload)?).is_err() {
            self.close(
                Some(Failure {
                    origin: FailureOrigin::Local,
                    code: FailureCode::ResourceLimit,
                }),
                None,
            );
        }
        Ok(())
    }

    /// Report a fully written frame; normal completion waits for local END delivery.
    pub fn sent(&mut self, kind: FrameType, id: u32) {
        tracing::trace!(
            function = "Session::sent",
            "Recording stream output completion"
        );
        if kind == FrameType::GoAway {
            self.goaway_written = true;
        }
        if kind == FrameType::End {
            if let Some(state) = self.streams.get_mut(&id) {
                state.local_end_written = !state.local_open;
            }
            self.complete(id);
        }
    }

    /// Terminate unfinished work once and optionally queue one safe connection ERROR.
    /// Callers permit a response only when framing and send integrity are trustworthy.
    /// A normal close is valid only after all accepted streams have completed.
    pub fn close(&mut self, failure: Option<Failure>, response: Option<ErrorCode>) {
        tracing::trace!(
            function = "Session::close",
            "Closing connection and failing unfinished streams"
        );
        if self.closed {
            return;
        }
        self.closed = true;
        let failure = failure.or(self.drain_failure);
        self.outbound.clear();
        let unfinished = failure.unwrap_or(Failure {
            origin: FailureOrigin::Local,
            code: FailureCode::ConnectionLost,
        });
        for (token, _) in std::mem::take(&mut self.pings) {
            self.events.push_back(Event::PingFinished {
                connection: self.handle.id().clone(),
                token,
                result: Err(unfinished),
            });
        }
        for (_, state) in std::mem::take(&mut self.streams) {
            self.events.push_back(Event::StreamClosed {
                stream: state.handle,
                outcome: StreamOutcome::Failed(unfinished),
            });
        }
        self.events.push_back(Event::ConnectionClosed {
            connection: self.handle.id().clone(),
            failure,
        });
        if let Some(code) = response
            && let Ok(payload) = ErrorPayload::new(code, "")
            && let Ok(frame) = frame(FrameType::Error, 0, payload.encode())
        {
            let _ = self.queue(frame);
        }
    }

    /// True after terminal connection closure; queued outcomes remain drainable.
    pub fn is_closed(&self) -> bool {
        tracing::trace!(
            function = "Session::is_closed",
            "Checking connection liveness"
        );
        self.closed
    }

    // A queued END is insufficient: successful output and remote END must both hold.
    pub(super) fn complete(&mut self, id: u32) {
        tracing::trace!(
            function = "Session::complete",
            "Checking both stream directions for completion"
        );
        if self
            .streams
            .get(&id)
            .is_some_and(|state| !state.remote_open && state.local_end_written)
            && let Some(state) = self.streams.remove(&id)
        {
            self.events.push_back(Event::StreamClosed {
                stream: state.handle,
                outcome: StreamOutcome::Completed,
            });
        }
    }

    // Isolate malformed or overflowing active streams and preserve unrelated work.
    pub(super) fn fail_stream(
        &mut self,
        id: u32,
        wire: ErrorCode,
        code: FailureCode,
    ) -> Result<(), SessionError> {
        tracing::trace!(
            function = "Session::fail_stream",
            "Terminating an affected stream"
        );
        self.terminate(
            id,
            StreamOutcome::Failed(Failure {
                origin: FailureOrigin::Local,
                code,
            }),
        );
        self.refuse(id, wire)
    }

    // Keep any unsent OPEN so remote numbering remains contiguous, but stop later production.
    pub(super) fn terminate(&mut self, id: u32, outcome: StreamOutcome) {
        tracing::trace!(
            function = "Session::terminate",
            "Releasing terminal stream production"
        );
        self.outbound.retain(|frame| {
            frame.header().stream_id() != id || frame.header().frame_type() == FrameType::Open
        });
        if let Some(state) = self.streams.remove(&id) {
            self.events.push_back(Event::StreamClosed {
                stream: state.handle,
                outcome,
            });
        }
    }
}
