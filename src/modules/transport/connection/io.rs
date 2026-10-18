//! Serialized frame output and verified frame input for the connection owner.
use super::*;

impl<C: Cryptography> Connection<C> {
    // Serialize exactly one frame at a time and commit its sequence only after full output.
    pub(super) fn write(&mut self, cx: &mut Context<'_>, crypto: &C, now: Instant) -> bool {
        tracing::trace!(
            function = "Connection::write",
            "Advancing serialized frame output"
        );
        if self.socket.is_none() {
            return false;
        }
        if !self.writer.is_busy() {
            let next = if let Some(session) = &mut self.session {
                session.pop_outbound()
            } else {
                self.handshake_output.pop_front()
            };
            let Some(mut frame) = next else {
                if self.closing {
                    self.release();
                }
                return false;
            };
            let kind = frame.header().frame_type();
            let id = frame.header().stream_id();
            let phase = if let Some(integrity) = &mut self.integrity {
                frame = match integrity.seal(frame, crypto) {
                    Ok(frame) => frame,
                    Err(error) => {
                        self.abort(Disposition::integrity(error));
                        return false;
                    }
                };
                FrameProtection::Tagged
            } else {
                FrameProtection::Untagged
            };
            if let Err(error) = self.writer.start(frame, phase) {
                self.abort(Disposition::framing(error));
                return false;
            }
            self.writing = Some((kind, id, phase));
        }
        let Some(socket) = self.socket.as_mut() else {
            return false;
        };
        match self.writer.poll_flush(socket.as_mut(), cx) {
            Poll::Pending => false,
            Poll::Ready(Err(error)) => {
                self.abort(Disposition::framing(error));
                false
            }
            Poll::Ready(Ok(())) => {
                if let Some((kind, id, phase)) = self.writing.take() {
                    if phase == FrameProtection::Tagged {
                        if let Some(integrity) = &mut self.integrity
                            && let Err(error) = integrity.sent()
                        {
                            self.abort(Disposition::integrity(error));
                            return false;
                        }
                    } else if !self.closing
                        && let Some(handshake) = &mut self.handshake
                        && let Err(error) = handshake.sent(kind)
                    {
                        self.fail(Disposition::handshake(error), now);
                        return false;
                    }
                    if let Some(session) = &mut self.session {
                        session.sent(kind, id);
                    }
                }
                true
            }
        }
    }

    // Do not consume post-AUTH frames until local AUTH output allows the key handoff.
    pub(super) fn read(
        &mut self,
        cx: &mut Context<'_>,
        now: Instant,
        registry: &Registry,
        identity: &mut impl Identity,
        crypto: &mut C,
    ) -> bool {
        tracing::trace!(
            function = "Connection::read",
            "Reading the next integrity-checked frame"
        );
        let phase = self
            .handshake
            .as_ref()
            .map_or(FrameProtection::Tagged, Handshake::receive_protection);
        if phase == FrameProtection::Tagged && self.integrity.is_none() {
            return false;
        }
        let Some(socket) = self.socket.as_mut() else {
            return false;
        };
        let frame = match self.reader.poll_frame(socket.as_mut(), cx, phase) {
            Poll::Pending => return false,
            Poll::Ready(Err(error)) => {
                self.fail(Disposition::framing(error), now);
                return true;
            }
            Poll::Ready(Ok(None)) => {
                self.abort(local_failure(FailureCode::ConnectionLost));
                return true;
            }
            Poll::Ready(Ok(Some(frame))) => frame,
        };
        if let Some(handshake) = &mut self.handshake {
            match handshake.receive(frame, identity, crypto) {
                Ok(Some(mut auth)) => {
                    auth.reservation = self.resources.outbound(auth.payload().len() + 12);
                    if auth.reservation.is_some() {
                        self.handshake_output.push_back(auth);
                    } else {
                        self.abort(local_failure(FailureCode::ResourceLimit));
                    }
                }
                Ok(None) => {}
                Err(error) => self.fail(Disposition::handshake(error), now),
            }
        } else if let Some(integrity) = &mut self.integrity {
            match integrity.verify(frame, crypto) {
                Err(error) => self.abort(Disposition::integrity(error)),
                Ok(verified) => {
                    if let Some(session) = &mut self.session {
                        let result = session.receive(verified, registry, now);
                        if session.is_closed() {
                            if self.writer.is_busy() {
                                self.release();
                            } else {
                                self.closing = true;
                                self.deadline = now.checked_add(self.timeout).unwrap_or(now);
                            }
                        }
                        let _ = result;
                    }
                }
            }
        }
        true
    }
}
