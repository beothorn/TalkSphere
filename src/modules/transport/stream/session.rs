//! Authenticated connection stream routing with queued consumer events.
//! The driver supplies verified frames and drains outbound frames through integrity/I/O.
use super::super::{
    api::{
        ConnectionRole, Event, Failure, FailureCode, FailureOrigin, PeerContext, Protocol,
        StreamOutcome,
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
};

/// Local operation rejection or a connection-level protocol violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionError {
    /// Remote stream numbering violates the connection history.
    Id(IdError),
    /// Caller is not registered for the requested protocol.
    Owner,
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
        let handle = ConnectionHandle::new();
        let context = PeerContext {
            connection: handle.id().clone(),
            role,
            local_identity,
            peer_identity,
        };
        Self {
            handle,
            context: context.clone(),
            ids: StreamIds::new(role),
            streams: BTreeMap::new(),
            events: VecDeque::from([Event::Authenticated(context)]),
            outbound: VecDeque::new(),
        }
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
        let protocol = registry.protocol(owner).ok_or(SessionError::Owner)?;
        let payload = Open::new(&protocol.id, protocol.version)
            .map_err(|_| SessionError::Protocol)?
            .encode();
        let id = self.ids.allocate().map_err(SessionError::Id)?;
        let handle = self.add(id, owner, protocol.clone())?;
        self.outbound
            .push_back(frame(FrameType::Open, id, payload)?);
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
        let queued: usize = self
            .outbound
            .iter()
            .map(|frame| frame.payload().len() + 28)
            .sum();
        if queued + bytes.len() + 28 > 4 * (MAX_PAYLOAD_BYTES + 28) {
            return Err(SessionError::Capacity);
        }
        self.outbound
            .push_back(frame(FrameType::Data, stream.wire_id().get(), bytes)?);
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
        state.local_open = false;
        self.outbound
            .push_back(frame(FrameType::End, stream.wire_id().get(), vec![])?);
        Ok(())
    }

    /// Report a fully written frame; normal completion waits for local END delivery.
    pub fn sent(&mut self, kind: FrameType, id: u32) {
        tracing::trace!(
            function = "Session::sent",
            "Recording stream output completion"
        );
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
    ) -> Result<(), SessionError> {
        tracing::trace!(
            function = "Session::receive",
            "Routing a verified stream frame"
        );
        self.receive_parts(
            frame.header().frame_type(),
            frame.header().stream_id(),
            frame.into_payload(),
            registry,
        )
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
        self.outbound.pop_front()
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
            if buffered + payload.len() > MAX_PAYLOAD_BYTES {
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
        if let Some(state) = self.streams.remove(&id) {
            let stream = state.handle;
            self.events.push_back(Event::StreamClosed {
                stream,
                outcome: StreamOutcome::Failed(Failure {
                    origin: FailureOrigin::Local,
                    code,
                }),
            });
        }
        self.refuse(id, wire)
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
        self.outbound
            .push_back(frame(FrameType::Error, id, payload)?);
        Ok(())
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
