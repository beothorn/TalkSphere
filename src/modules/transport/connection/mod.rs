//! Single-socket composition of framing, handshake, integrity and stream state.
//! A parent driver supplies monotonic time, shared dependencies and registry, and polls fairly.
use super::{
    api::{ConnectionRole, Event, ExpectedPeer, Failure, FailureCode, FailureOrigin},
    codec::{
        FrameProtection, FrameType,
        control::{ErrorCode, ErrorPayload},
    },
    config::TransportConfig,
    failure::Disposition,
    framing::{FrameReader, FrameWriter, WireFrame},
    handles::ConnectionHandle,
    handshake::Handshake,
    integrity::Integrity,
    ports::{crypto::Cryptography, identity::Identity, io::Socket},
    stream::{registry::Registry, session::Session},
};
use std::{
    collections::VecDeque,
    task::{Context, Poll},
    time::{Duration, Instant},
};
mod io;

/// Trusted setup inputs, independent of peer claims.
#[derive(Clone, Copy)]
pub struct Options {
    /// TCP role determines proof labels and stream parity.
    pub role: ConnectionRole,
    /// Known-contact pins remain mandatory during handshake.
    pub expected_peer: ExpectedPeer,
}

/// One owned socket and its transient credentials; dropping closes/releases them.
pub struct Connection<C: Cryptography> {
    resources: super::resources::Resources,
    admission: Option<super::resources::Reservation>,
    handle: ConnectionHandle,
    socket: Option<Box<dyn Socket>>,
    reader: FrameReader,
    writer: FrameWriter,
    handshake: Option<Handshake<C>>,
    integrity: Option<Integrity<C::IntegrityKey>>,
    session: Option<Session>,
    handshake_output: VecDeque<WireFrame>,
    writing: Option<(FrameType, u32, FrameProtection)>,
    events: VecDeque<Event>,
    deadline: Instant,
    timeout: Duration,
    closing: bool,
}

impl<C: Cryptography> Connection<C> {
    /// Own a socket, request fresh handshake material and queue HELLO.
    /// # Errors
    /// Returns a terminal disposition on credential/entropy/deadline failure and closes the socket.
    pub fn begin(
        socket: Box<dyn Socket>,
        options: Options,
        config: TransportConfig,
        now: Instant,
        identity: &impl Identity,
        crypto: &mut C,
    ) -> Result<Self, Disposition> {
        Self::begin_shared(
            socket,
            options,
            config,
            now,
            identity,
            crypto,
            super::resources::Resources::new(*config.limits()),
        )
    }

    /// Admit a socket against shared instance-wide limits before credential allocation.
    /// # Errors
    /// Returns resource failure without starting authentication when capacity is full.
    pub fn begin_shared(
        socket: Box<dyn Socket>,
        options: Options,
        config: TransportConfig,
        now: Instant,
        identity: &impl Identity,
        crypto: &mut C,
        resources: super::resources::Resources,
    ) -> Result<Self, Disposition> {
        tracing::trace!(
            function = "Connection::begin",
            "Beginning an owned connection handshake"
        );
        let mut socket = socket;
        let Some(admission) = resources.connection() else {
            let _ = socket.close();
            return Err(local_failure(FailureCode::ResourceLimit));
        };
        let Some(hello_reservation) = resources.outbound(112) else {
            let _ = socket.close();
            return Err(local_failure(FailureCode::ResourceLimit));
        };
        let Some(deadline) = now.checked_add(config.handshake_timeout()) else {
            let _ = socket.close();
            return Err(local_failure(FailureCode::Internal));
        };
        let (handshake, mut hello) =
            match Handshake::begin(options.role, options.expected_peer, identity, crypto) {
                Ok(value) => value,
                Err(error) => {
                    let _ = socket.close();
                    return Err(Disposition::handshake(error));
                }
            };
        hello.reservation = Some(hello_reservation);
        Ok(Self {
            resources,
            admission: Some(admission),
            handle: ConnectionHandle::new(),
            socket: Some(socket),
            reader: FrameReader::default(),
            writer: FrameWriter::default(),
            handshake: Some(handshake),
            integrity: None,
            session: None,
            handshake_output: VecDeque::from([hello]),
            writing: None,
            events: VecDeque::new(),
            deadline,
            timeout: config.handshake_timeout(),
            closing: false,
        })
    }

    /// Borrow authority for this exact socket instance before or after authentication.
    pub fn handle(&self) -> &ConnectionHandle {
        tracing::trace!(
            function = "Connection::handle",
            "Reading socket instance authority"
        );
        &self.handle
    }

    /// Borrow authenticated stream operations only after both handshake directions succeed.
    pub fn session(&mut self) -> Option<&mut Session> {
        tracing::trace!(
            function = "Connection::session",
            "Accessing authenticated connection operations"
        );
        self.session.as_mut()
    }

    /// Drain queued outcomes; authentication and consumer callbacks never run inside socket I/O.
    pub fn pop_event(&mut self) -> Option<Event> {
        tracing::trace!(
            function = "Connection::pop_event",
            "Delivering connection observations"
        );
        self.events
            .pop_front()
            .or_else(|| self.session.as_mut().and_then(Session::pop_event))
    }

    /// Earliest required wakeup; the parent registers it with its injected clock.
    pub fn next_deadline(&self) -> Option<Instant> {
        tracing::trace!(
            function = "Connection::next_deadline",
            "Selecting connection deadline"
        );
        self.socket.as_ref()?;
        if self.handshake.is_some() || self.closing {
            return Some(self.deadline);
        }
        self.session.as_ref().and_then(Session::next_deadline)
    }

    /// Advance bounded I/O work; Pending relies on socket readiness or the parent clock.
    /// Ready means the socket is closed, while final events remain available.
    pub fn poll(
        &mut self,
        cx: &mut Context<'_>,
        now: Instant,
        registry: &Registry,
        identity: &mut impl Identity,
        crypto: &mut C,
    ) -> Poll<()> {
        tracing::trace!(
            function = "Connection::poll",
            "Driving connection I/O and deadlines"
        );
        if self.socket.is_none() {
            return Poll::Ready(());
        }
        if (self.handshake.is_some() || self.closing) && now >= self.deadline {
            self.abort(local_failure(FailureCode::DeadlineExceeded));
            return Poll::Ready(());
        }
        if let Some(session) = &mut self.session {
            session.tick(now);
        }
        for _ in 0..32 {
            let mut progressed = self.write(cx, crypto, now);
            self.establish();
            if self.socket.is_none() {
                return Poll::Ready(());
            }
            if !self.closing {
                progressed |= self.read(cx, now, registry, identity, crypto);
            }
            self.establish();
            if self.session.as_ref().is_some_and(Session::ready_to_close) && !self.writer.is_busy()
            {
                if let Some(session) = &mut self.session {
                    session.close(None, None);
                }
                self.release();
            }
            if self.socket.is_none() {
                return Poll::Ready(());
            }
            if !progressed {
                return Poll::Pending;
            }
        }
        cx.waker().wake_by_ref();
        Poll::Pending
    }

    /// Explicitly close the socket and fail unfinished work without waiting for peer activity.
    pub fn close(&mut self) {
        tracing::trace!(
            function = "Connection::close",
            "Closing an owned socket on local request"
        );
        self.abort(local_failure(FailureCode::ConnectionLost));
    }

    // Transfer verified facts and secrets exactly once, preserving the original handle.
    fn establish(&mut self) {
        tracing::trace!(
            function = "Connection::establish",
            "Checking authentication handoff readiness"
        );
        if let Some(established) = self
            .handshake
            .as_mut()
            .and_then(Handshake::take_established)
        {
            self.session = Some(Session::with_handle(
                self.handle.clone(),
                established.role,
                established.local_identity,
                established.peer_identity,
            ));
            if let Some(session) = &mut self.session {
                session.set_resources(self.resources.clone());
            }
            self.integrity = Some(Integrity::new(established.keys));
            self.handshake = None;
        }
    }

    // Preserve consumer outcomes before releasing socket, buffers and transient key capabilities.
    fn abort(&mut self, disposition: Disposition) {
        tracing::trace!(
            function = "Connection::abort",
            "Failing unfinished connection work"
        );
        if self.socket.is_none() {
            return;
        }
        if let Some(session) = &mut self.session {
            session.close(Some(disposition.failure), None);
        } else if !self.closing {
            self.events.push_back(Event::ConnectionClosed {
                connection: self.handle.id().clone(),
                failure: Some(disposition.failure),
            });
        }
        self.release();
    }

    // Best-effort errors use only known idle output state and have a finite write deadline.
    fn fail(&mut self, disposition: Disposition, now: Instant) {
        tracing::trace!(
            function = "Connection::fail",
            "Applying safe connection failure response policy"
        );
        let safe = !self.writer.is_busy()
            && (self.integrity.is_some()
                || self
                    .handshake
                    .as_ref()
                    .is_some_and(|h| h.send_protection() == FrameProtection::Untagged));
        let disposition = disposition.with_safe_output(safe);
        let Some(code) = disposition.response else {
            self.abort(disposition);
            return;
        };
        let Some(deadline) = now.checked_add(self.timeout) else {
            self.abort(disposition);
            return;
        };
        self.deadline = deadline;
        self.closing = true;
        self.handshake_output.clear();
        if let Some(session) = &mut self.session {
            session.close(Some(disposition.failure), Some(code));
        } else {
            self.events.push_back(Event::ConnectionClosed {
                connection: self.handle.id().clone(),
                failure: Some(disposition.failure),
            });
            if let Some(mut frame) = error_frame(code) {
                frame.reservation = self.resources.outbound(frame.payload().len() + 12);
                if frame.reservation.is_some() {
                    self.handshake_output.push_back(frame);
                }
            }
        }
    }

    // Drop owned state even if close itself reports an OS error; no peer acknowledgement is needed.
    fn release(&mut self) {
        tracing::trace!(
            function = "Connection::release",
            "Releasing socket buffers and session secrets"
        );
        if let Some(mut socket) = self.socket.take() {
            let _ = socket.close();
        }
        self.admission = None;
        self.handshake = None;
        self.integrity = None;
        self.handshake_output.clear();
        if let Some(session) = &mut self.session {
            while session.pop_outbound().is_some() {}
        }
        self.reader = FrameReader::default();
        self.writer = FrameWriter::default();
        self.writing = None;
    }
}

impl<C: Cryptography> Drop for Connection<C> {
    // Dropping an unpolled or failed connection still closes the socket and releases keys.
    fn drop(&mut self) {
        self.release();
    }
}

// Local terminal conditions that cannot safely or usefully produce wire output.
fn local_failure(code: FailureCode) -> Disposition {
    tracing::trace!(
        function = "connection::local_failure",
        "Classifying a local connection termination"
    );
    Disposition {
        failure: Failure {
            origin: FailureOrigin::Local,
            code,
        },
        response: None,
    }
}

// Construct a diagnostic-free connection ERROR from a validated registry code.
fn error_frame(code: ErrorCode) -> Option<WireFrame> {
    tracing::trace!(
        function = "connection::error_frame",
        "Building a safe connection error response"
    );
    let payload = ErrorPayload::new(code, "").ok()?.encode();
    let header = super::codec::FrameHeader::new(FrameType::Error, 0, payload.len() as u32).ok()?;
    WireFrame::new(header, payload, None).ok()
}

#[cfg(test)]
mod tests;
