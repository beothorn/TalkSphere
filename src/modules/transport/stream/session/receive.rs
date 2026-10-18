//! Focused session dispatch for already verified frames; state errors retain their required scope.
use super::*;

impl Session {
    // Consume the OPEN ID before payload validation or registration lookup.
    pub(super) fn receive_parts(
        &mut self,
        kind: FrameType,
        id: u32,
        payload: Vec<u8>,
        registry: &Registry,
    ) -> Result<(), SessionError> {
        tracing::trace!(
            function = "Session::receive_parts",
            "Checking stream routing state"
        );
        if kind == FrameType::GoAway && id == 0 {
            return self.receive_goaway(&payload);
        }
        if matches!(kind, FrameType::Ping | FrameType::Pong) && id == 0 {
            return self.receive_ping(kind, &payload);
        }
        if kind == FrameType::Error && id == 0 {
            let error = ErrorPayload::decode(&payload).map_err(|_| SessionError::Protocol)?;
            self.close(
                Some(Failure {
                    origin: FailureOrigin::Peer,
                    code: failure_code(error.code()),
                }),
                None,
            );
            return Ok(());
        }
        if kind != FrameType::Open && id != 0 {
            if !self.ids.was_created(id) {
                return Err(SessionError::Protocol);
            }
            if !self.streams.contains_key(&id) {
                return Ok(());
            }
        }
        if matches!(kind, FrameType::Cancel | FrameType::Error) && id != 0 {
            let diagnostic = ErrorPayload::decode(&payload).map_err(|_| SessionError::Protocol)?;
            let code = diagnostic.code();
            let outcome = if kind == FrameType::Cancel
                && matches!(
                    code,
                    ErrorCode::Cancelled | ErrorCode::ResourceLimit | ErrorCode::InternalError
                ) {
                StreamOutcome::Cancelled {
                    origin: FailureOrigin::Peer,
                    reason: match code {
                        ErrorCode::ResourceLimit => CancellationReason::ResourceLimit,
                        ErrorCode::InternalError => CancellationReason::InternalError,
                        _ => CancellationReason::Cancelled,
                    },
                }
            } else {
                StreamOutcome::Failed(Failure {
                    origin: FailureOrigin::Peer,
                    code: failure_code(code),
                })
            };
            self.terminate(id, outcome);
            return Ok(());
        }
        if kind == FrameType::End {
            let state = self.streams.get_mut(&id).ok_or(SessionError::Protocol)?;
            if !payload.is_empty() || !state.remote_open {
                return self.fail_stream(id, ErrorCode::StreamStateError, FailureCode::StreamState);
            }
            state.remote_open = false;
            self.events.push_back(Event::RemoteEnd {
                stream: state.handle.clone(),
            });
            self.complete(id);
            return Ok(());
        }
        if kind == FrameType::Data {
            let state = self.streams.get(&id).ok_or(SessionError::Protocol)?;
            if !state.remote_open {
                return self.fail_stream(id, ErrorCode::StreamStateError, FailureCode::StreamState);
            }
            let stream = state.handle.clone();
            if payload.is_empty() || payload.len() > MAX_PAYLOAD_BYTES {
                return self.fail_stream(id, ErrorCode::StreamStateError, FailureCode::StreamState);
            }
            let buffered: usize = self
                .events
                .iter()
                .filter_map(|event| match event {
                    Event::Data { stream, bytes } if stream.wire_id().get() == id => {
                        Some(bytes.len())
                    }
                    _ => None,
                })
                .sum();
            if buffered + payload.len() > self.resources.limits().buffered_bytes_per_stream {
                return self.fail_stream(id, ErrorCode::ResourceLimit, FailureCode::ResourceLimit);
            }
            self.events.push_back(Event::Data {
                stream,
                bytes: payload,
            });
            return Ok(());
        }
        if kind != FrameType::Open {
            return Err(SessionError::Protocol);
        }
        self.ids.accept(id).map_err(SessionError::Id)?;
        if self.goaway_sent || self.peer_goaway.is_some() {
            return self.refuse(id, ErrorCode::RefusedStream);
        }
        if self.streams.len() >= self.resources.limits().streams_per_connection {
            return self.refuse(id, ErrorCode::ResourceLimit);
        }
        let selection = match Open::decode(&payload) {
            Ok(value) => value,
            Err(_) => return self.refuse(id, ErrorCode::ProtocolError),
        };
        let Some(owner) = registry.resolve(selection) else {
            return self.refuse(id, ErrorCode::UnsupportedProtocol);
        };
        self.add(
            id,
            owner,
            Protocol {
                id: selection.id().to_owned(),
                version: selection.version(),
            },
        )?;
        Ok(())
    }
}
