//! Authenticated connection stream routing with queued consumer events.
//! The driver supplies verified frames and drains outbound frames through integrity/I/O.
use super::super::{
    api::{
        CancellationReason, ConnectionRole, Event, Failure, FailureCode, FailureOrigin,
        PeerContext, Protocol, StreamOutcome,
    },
    codec::{
        FrameHeader, FrameType, MAX_PAYLOAD_BYTES,
        control::{ErrorCode, ErrorPayload},
        open::Open,
    },
    framing::{FramingError, WireFrame},
    handles::{ConnectionHandle, StreamHandle, StreamOwner},
    integrity::VerifiedFrame,
    ports::identity::IdentityId,
};
use super::{
    ids::{IdError, StreamIds},
    registry::Registry,
};
use std::{
    collections::{BTreeMap, VecDeque},
    num::NonZeroU32,
    time::Instant,
};

/// Local operation rejection or a connection-level protocol violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionError {
    /// Remote stream numbering violates the connection history.
    Id(IdError),
    /// Caller is not registered for the requested protocol.
    Owner,
    /// Connection has already terminated.
    Closed,
    /// GOAWAY forbids creating additional local streams.
    Draining,
    /// Caller supplied a stream from another connection or owner.
    Authority(super::super::TransportError),
    /// DATA must contain one to 65,536 bytes.
    InvalidData,
    /// A bounded queue cannot currently admit more bytes.
    Capacity,
    /// Frame is not supported in this state.
    Protocol,
    /// Internal frame construction failed.
    Framing(FramingError),
}

pub(super) struct StreamState {
    handle: StreamHandle,
    local_open: bool,
    remote_open: bool,
    local_end_written: bool,
}

/// One authenticated connection; no socket, key or application callback is retained.
pub struct Session {
    last_output: Option<u32>,
    resources: super::super::resources::Resources,
    accepted_remote: u32,
    peer_goaway: Option<u32>,
    drain_failure: Option<Failure>,
    goaway_sent: bool,
    goaway_written: bool,
    pings: BTreeMap<[u8; 8], (Instant, Instant)>,
    now: Option<Instant>,
    pub(super) closed: bool,
    pub(super) handle: ConnectionHandle,
    pub(super) context: PeerContext,
    pub(super) ids: StreamIds,
    pub(super) streams: BTreeMap<u32, StreamState>,
    pub(super) events: VecDeque<Event>,
    pub(super) outbound: VecDeque<WireFrame>,
}

impl Session {
    /// Create state from facts supplied by successful handshake composition.
    /// Emits an authenticated event; callers must never supply unverified peer claims.
    pub fn new(
        role: ConnectionRole,
        local_identity: IdentityId,
        peer_identity: IdentityId,
    ) -> Self {
        tracing::trace!(
            function = "Session::new",
            "Starting authenticated stream routing"
        );
        Self::with_handle(ConnectionHandle::new(), role, local_identity, peer_identity)
    }

    // Preserve the authority issued before authentication when handing off driver state.
    pub(crate) fn with_handle(
        handle: ConnectionHandle,
        role: ConnectionRole,
        local_identity: IdentityId,
        peer_identity: IdentityId,
    ) -> Self {
        tracing::trace!(
            function = "Session::with_handle",
            "Attaching authenticated state to connection authority"
        );
        let context = PeerContext {
            connection: handle.id().clone(),
            role,
            local_identity,
            peer_identity,
        };
        Self {
            last_output: None,
            resources: super::super::resources::Resources::new(
                super::super::config::ResourceLimits {
                    connections: 1,
                    streams_per_connection: 256,
                    outbound_bytes: 4 * (MAX_PAYLOAD_BYTES + 28),
                    buffered_bytes_per_stream: MAX_PAYLOAD_BYTES,
                },
            ),
            accepted_remote: 0,
            peer_goaway: None,
            drain_failure: None,
            goaway_sent: false,
            goaway_written: false,
            pings: BTreeMap::new(),
            now: None,
            closed: false,
            handle,
            context: context.clone(),
            ids: StreamIds::new(role),
            streams: BTreeMap::new(),
            events: VecDeque::from([Event::Authenticated(context)]),
            outbound: VecDeque::new(),
        }
    }

    // Bound metadata events as well as payload bytes when composition stops consuming.
    fn event_limit(&self) -> usize {
        tracing::trace!(
            function = "Session::event_limit",
            "Calculating finite consumer event capacity"
        );
        self.resources
            .limits()
            .streams_per_connection
            .saturating_mul(4)
            .saturating_add(130)
    }

    // Attach the parent's shared budgets before the session accepts any stream work.
    pub(crate) fn set_resources(&mut self, resources: super::super::resources::Resources) {
        tracing::trace!(
            function = "Session::set_resources",
            "Attaching instance-wide resource limits"
        );
        self.resources = resources;
    }

    // All control and application output shares one finite serialized-byte budget.
    fn queue(&mut self, mut output: WireFrame) -> Result<(), SessionError> {
        tracing::trace!(
            function = "Session::queue",
            "Admitting bounded session output"
        );
        output.reservation = Some(
            self.resources
                .outbound(output.payload().len() + 28)
                .ok_or(SessionError::Capacity)?,
        );
        self.outbound.push_back(output);
        Ok(())
    }

    /// Borrow this connection's control capability for trusted composition.
    pub fn handle(&self) -> &ConnectionHandle {
        tracing::trace!(function = "Session::handle", "Reading connection authority");
        &self.handle
    }

    /// Open the owner's registered protocol, creating state before exposing OPEN.
    /// # Errors
    /// Rejects unknown owner authority, exhausted IDs and frame construction failure.
    pub fn open(
        &mut self,
        registry: &Registry,
        owner: &StreamOwner,
    ) -> Result<StreamHandle, SessionError> {
        tracing::trace!(function = "Session::open", "Opening a locally owned stream");
        if self.closed {
            return Err(SessionError::Closed);
        }
        if self.goaway_sent || self.peer_goaway.is_some() {
            return Err(SessionError::Draining);
        }
        let protocol = registry.protocol(owner).ok_or(SessionError::Owner)?;
        let payload = Open::new(&protocol.id, protocol.version)
            .map_err(|_| SessionError::Protocol)?
            .encode();
        if self.events.len() >= self.event_limit()
            || self.streams.len() >= self.resources.limits().streams_per_connection
        {
            return Err(SessionError::Capacity);
        }
        let reservation = self
            .resources
            .outbound(payload.len() + 28)
            .ok_or(SessionError::Capacity)?;
        let id = match self.ids.allocate() {
            Ok(id) => id,
            Err(error) => {
                self.drain(super::super::api::ShutdownReason::IdExhausted)?;
                return Err(SessionError::Id(error));
            }
        };
        let handle = self.add(id, owner, protocol.clone())?;
        let mut output = frame(FrameType::Open, id, payload)?;
        output.reservation = Some(reservation);
        self.outbound.push_back(output);
        Ok(handle)
    }

    /// Queue ordered bytes for one owned stream; DATA boundaries are not messages.
    /// # Errors
    /// Rejects invalid authority, inactive streams, empty/oversized data or full queues.
    pub fn send(
        &mut self,
        owner: &StreamOwner,
        stream: &StreamHandle,
        bytes: Vec<u8>,
    ) -> Result<(), SessionError> {
        tracing::trace!(function = "Session::send", "Queueing ordered stream bytes");
        if self.closed {
            return Err(SessionError::Closed);
        }
        stream
            .authorize(self.handle.id(), owner)
            .map_err(SessionError::Authority)?;
        if !self
            .streams
            .get(&stream.wire_id().get())
            .is_some_and(|state| state.local_open)
        {
            return Err(SessionError::Protocol);
        }
        if bytes.is_empty() || bytes.len() > MAX_PAYLOAD_BYTES {
            return Err(SessionError::InvalidData);
        }
        self.queue(frame(FrameType::Data, stream.wire_id().get(), bytes)?)?;
        Ok(())
    }

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

    /// Dispatch a frame only after successful integrity verification.
    /// # Errors
    /// Reports connection violations; malformed/unsupported OPEN is refused per stream.
    pub fn receive(
        &mut self,
        frame: VerifiedFrame,
        registry: &Registry,
        now: Instant,
    ) -> Result<(), SessionError> {
        tracing::trace!(
            function = "Session::receive",
            "Routing a verified stream frame"
        );
        if self.closed {
            return Err(SessionError::Closed);
        }
        self.tick(now);
        if self.events.len() >= self.event_limit() {
            self.close(
                Some(Failure {
                    origin: FailureOrigin::Local,
                    code: FailureCode::ResourceLimit,
                }),
                None,
            );
            return Err(SessionError::Capacity);
        }
        let result = self.receive_parts(
            frame.header().frame_type(),
            frame.header().stream_id(),
            frame.into_payload(),
            registry,
        );
        if result.is_err() {
            self.close(
                Some(Failure {
                    origin: FailureOrigin::Local,
                    code: if result == Err(SessionError::Capacity) {
                        FailureCode::ResourceLimit
                    } else {
                        FailureCode::ProtocolViolation
                    },
                }),
                Some(if result == Err(SessionError::Capacity) {
                    ErrorCode::ResourceLimit
                } else {
                    ErrorCode::ProtocolError
                }),
            );
        }
        result
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

    /// Remove the next consumer event; no application code executes inside routing.
    pub fn pop_event(&mut self) -> Option<Event> {
        tracing::trace!(
            function = "Session::pop_event",
            "Delivering a queued consumer event"
        );
        self.events.pop_front()
    }

    /// Transfer the next unsealed frame to the single connection writer.
    pub fn pop_outbound(&mut self) -> Option<WireFrame> {
        tracing::trace!(
            function = "Session::pop_outbound",
            "Transferring queued connection output"
        );
        self.next_output()
    }

    // Consume the OPEN ID before payload validation or registration lookup.
    fn receive_parts(
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

    // Install scoped ownership before queueing a new-stream event.
    fn add(
        &mut self,
        id: u32,
        owner: &StreamOwner,
        protocol: Protocol,
    ) -> Result<StreamHandle, SessionError> {
        tracing::trace!(
            function = "Session::add",
            "Installing an owned stream before dispatch"
        );
        if !self.ids.is_local(id) {
            self.accepted_remote = id;
        }
        let wire_id = NonZeroU32::new(id).ok_or(SessionError::Protocol)?;
        let stream = StreamHandle::new(&self.handle, owner, wire_id);
        self.streams.insert(
            id,
            StreamState {
                handle: stream.clone(),
                local_open: true,
                remote_open: true,
                local_end_written: false,
            },
        );
        self.events.push_back(Event::StreamOpened {
            context: self.context.clone(),
            stream: stream.clone(),
            protocol,
        });
        Ok(stream)
    }

    // A queued END is insufficient: successful output and remote END must both hold.
    fn complete(&mut self, id: u32) {
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
    fn fail_stream(
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
    fn terminate(&mut self, id: u32, outcome: StreamOutcome) {
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

    // Refused OPENs remain consumed without retaining a terminal stream object.
    fn refuse(&mut self, id: u32, code: ErrorCode) -> Result<(), SessionError> {
        tracing::trace!(
            function = "Session::refuse",
            "Refusing a stream without affecting its peers"
        );
        let payload = ErrorPayload::new(code, "")
            .map_err(|_| SessionError::Protocol)?
            .encode();
        self.queue(frame(FrameType::Error, id, payload)?)?;
        Ok(())
    }
}

// Preserve peer error codes as typed local outcomes, never interpret diagnostic text.
pub(crate) fn failure_code(code: ErrorCode) -> FailureCode {
    tracing::trace!(
        function = "session::failure_code",
        "Mapping a peer failure code"
    );
    match code {
        ErrorCode::AuthenticationFailed => FailureCode::AuthenticationFailed,
        ErrorCode::FrameSizeError => FailureCode::FrameTooLarge,
        ErrorCode::UnsupportedProtocol => FailureCode::UnsupportedProtocol,
        ErrorCode::StreamStateError => FailureCode::StreamState,
        ErrorCode::RefusedStream => FailureCode::Refused,
        ErrorCode::InternalError => FailureCode::Internal,
        ErrorCode::IdExhausted => FailureCode::IdExhausted,
        ErrorCode::ResourceLimit => FailureCode::ResourceLimit,
        ErrorCode::Cancelled => FailureCode::Cancelled,
        ErrorCode::ProtocolError | ErrorCode::NoError => FailureCode::ProtocolViolation,
    }
}

// Construct a bounded unsealed frame; the owning connection supplies its tag later.
fn frame(kind: FrameType, id: u32, payload: Vec<u8>) -> Result<WireFrame, SessionError> {
    tracing::trace!(
        function = "session::frame",
        "Building bounded stream output"
    );
    let header = FrameHeader::new(kind, id, payload.len() as u32)
        .map_err(|e| SessionError::Framing(FramingError::Header(e)))?;
    WireFrame::new(header, payload, None).map_err(SessionError::Framing)
}

#[cfg(test)]
#[path = "session/tests.rs"]
mod tests;

mod ping;

mod drain;

mod scheduler;
