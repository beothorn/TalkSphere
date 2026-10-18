//! Authenticated connection liveness operations with caller-supplied monotonic time.
use super::*;
use std::time::{Duration, Instant};

impl Session {
    /// Queue a unique outstanding token with a positive, representable deadline.
    /// # Errors
    /// Rejects closed sessions, duplicate tokens, invalid timeouts or 64 pending pings.
    pub fn ping(
        &mut self,
        token: [u8; 8],
        timeout: Duration,
        now: Instant,
    ) -> Result<(), SessionError> {
        tracing::trace!(
            function = "Session::ping",
            "Queueing a timed connection probe"
        );
        if self.closed {
            return Err(SessionError::Closed);
        }
        if timeout.is_zero() || self.pings.contains_key(&token) {
            return Err(SessionError::Protocol);
        }
        if self.pings.len() >= 64 {
            return Err(SessionError::Capacity);
        }
        let deadline = now.checked_add(timeout).ok_or(SessionError::Protocol)?;
        self.pings.insert(token, (now, deadline));
        self.outbound
            .push_back(frame(FrameType::Ping, 0, token.to_vec())?);
        Ok(())
    }

    /// Expire probes at their deadlines; a timeout never reports successful liveness.
    pub fn tick(&mut self, now: Instant) {
        tracing::trace!(
            function = "Session::tick",
            "Expiring connection probe deadlines"
        );
        self.now = Some(now);
        let expired: Vec<_> = self
            .pings
            .iter()
            .filter(|(_, (_, deadline))| *deadline <= now)
            .map(|(token, _)| *token)
            .collect();
        for token in expired {
            self.pings.remove(&token);
            self.events.push_back(Event::PingFinished {
                connection: self.handle.id().clone(),
                token,
                result: Err(Failure {
                    origin: FailureOrigin::Local,
                    code: FailureCode::DeadlineExceeded,
                }),
            });
        }
    }

    /// Return the earliest outstanding deadline for the injected clock's wakeup.
    pub fn next_deadline(&self) -> Option<Instant> {
        tracing::trace!(
            function = "Session::next_deadline",
            "Finding the next connection timer"
        );
        self.pings.values().map(|(_, deadline)| *deadline).min()
    }

    // Echo each PING once; match only pending PONGs and retain unrelated probes.
    pub(super) fn receive_ping(
        &mut self,
        kind: FrameType,
        payload: &[u8],
    ) -> Result<(), SessionError> {
        tracing::trace!(
            function = "Session::receive_ping",
            "Processing an authenticated probe frame"
        );
        let token = super::super::super::codec::control::PingToken::decode(payload)
            .map_err(|_| SessionError::Protocol)?
            .encode();
        if kind == FrameType::Ping {
            self.outbound
                .push_back(frame(FrameType::Pong, 0, token.to_vec())?);
        } else if let Some((started, _)) = self.pings.remove(&token) {
            let elapsed = self
                .now
                .and_then(|now| now.checked_duration_since(started))
                .ok_or(SessionError::Protocol)?;
            self.events.push_back(Event::PingFinished {
                connection: self.handle.id().clone(),
                token,
                result: Ok(elapsed),
            });
        }
        Ok(())
    }
}
