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

mod lifecycle;

mod receive;
