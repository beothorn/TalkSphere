//! GOAWAY bounds and draining, independent of transport socket shutdown mechanics.
use super::*;
use crate::modules::transport::{api::ShutdownReason, codec::control::GoAway};

impl Session {
    /// Begin graceful shutdown, retaining already accepted streams.
    /// # Errors
    /// Rejects closed sessions or internal payload construction failure.
    pub fn drain(&mut self, reason: ShutdownReason) -> Result<(), SessionError> {
        tracing::trace!(
            function = "Session::drain",
            "Starting graceful connection drain"
        );
        if self.closed {
            return Err(SessionError::Closed);
        }
        if self.goaway_sent {
            return Ok(());
        }
        let code = match reason {
            ShutdownReason::Normal => ErrorCode::NoError,
            ShutdownReason::IdExhausted => ErrorCode::IdExhausted,
            ShutdownReason::ResourceLimit => ErrorCode::ResourceLimit,
            ShutdownReason::InternalError => ErrorCode::InternalError,
        };
        let payload = GoAway::new(self.accepted_remote, code, "")
            .map_err(|_| SessionError::Protocol)?
            .encode();
        self.queue(frame(FrameType::GoAway, 0, payload)?)?;
        self.goaway_sent = true;
        Ok(())
    }

    /// True only when accepted streams and queued output have drained after GOAWAY output.
    /// The connection driver may then close its socket normally.
    pub fn ready_to_close(&self) -> bool {
        tracing::trace!(
            function = "Session::ready_to_close",
            "Checking graceful drain completion"
        );
        !self.closed && self.goaway_written && self.streams.is_empty() && self.outbound.is_empty()
    }

    // Validate decreasing peer acceptance bounds before changing stream outcomes.
    pub(super) fn receive_goaway(&mut self, payload: &[u8]) -> Result<(), SessionError> {
        tracing::trace!(
            function = "Session::receive_goaway",
            "Validating peer graceful shutdown bounds"
        );
        let goaway = GoAway::decode(payload).map_err(|_| SessionError::Protocol)?;
        let last = goaway.last_accepted_remote_stream_id();
        if (last != 0 && !self.ids.is_local(last))
            || last > self.ids.greatest_local()
            || self.peer_goaway.is_some_and(|previous| last > previous)
        {
            return Err(SessionError::Protocol);
        }
        self.peer_goaway = Some(last);
        if goaway.code() != ErrorCode::NoError {
            self.drain_failure = Some(Failure {
                origin: FailureOrigin::Peer,
                code: failure_code(goaway.code()),
            });
        }
        let refused: Vec<_> = self
            .streams
            .keys()
            .copied()
            .filter(|&id| self.ids.is_local(id) && id > last)
            .collect();
        for id in refused {
            self.terminate(
                id,
                StreamOutcome::Failed(Failure {
                    origin: FailureOrigin::Peer,
                    code: FailureCode::Refused,
                }),
            );
        }
        self.drain(ShutdownReason::Normal)
    }
}
