//! Local request admission and capability routing; peer messages never enter this interface.
use super::*;
use crate::modules::transport::ports::io::Socket;

impl<S: SocketProvider, T: Clock, I: Identity, C: Cryptography> Driver<S, T, I, C> {
    // Route validated control requests to their owning connection without consumer callbacks.
    pub(super) fn request(&mut self, request: Request) -> Result<Accepted, TransportError> {
        tracing::trace!(
            function = "Driver::request",
            "Routing local Transport control"
        );
        match request {
            Request::RegisterProtocol { protocol } => {
                if self.registrations >= 256 {
                    return Err(TransportError::Backpressure);
                }
                let owner = self
                    .registry
                    .register(protocol)
                    .map_err(|_| TransportError::InvalidRequest)?;
                self.registrations += 1;
                Ok(Accepted::Registration(owner))
            }
            Request::Connect(request) => {
                if self.connections.len() + self.pending.len() >= self.config.limits().connections {
                    return Err(TransportError::Backpressure);
                }
                let reservation = self
                    .resources
                    .connection()
                    .ok_or(TransportError::Backpressure)?;
                let deadline = self
                    .dependencies
                    .clock
                    .now()
                    .checked_add(self.config.handshake_timeout())
                    .ok_or(TransportError::InvalidRequest)?;
                let attempt = self
                    .dependencies
                    .sockets
                    .connect(request.endpoint())
                    .map_err(|e| TransportError::Io(e.kind()))?;
                let handle = ConnectionHandle::new();
                self.pending.push(Pending {
                    handle: handle.clone(),
                    attempt,
                    expected: request.expected_peer(),
                    deadline,
                    reservation,
                });
                Ok(Accepted::Connection(handle))
            }
            Request::Accept {
                socket,
                expected_peer,
            } => self.accept(socket, expected_peer),
            Request::Listen { endpoint } => {
                if self.listeners.len() >= self.config.limits().connections {
                    return Err(TransportError::Backpressure);
                }
                let socket = self
                    .dependencies
                    .sockets
                    .listen(endpoint)
                    .map_err(|e| TransportError::Io(e.kind()))?;
                let local_addr = socket
                    .local_addr()
                    .map_err(|e| TransportError::Io(e.kind()))?;
                let handle = ListenerHandle::new();
                self.listeners.push((handle.clone(), socket));
                Ok(Accepted::Listener { handle, local_addr })
            }
            Request::StopListening { listener } => {
                let index = self
                    .listeners
                    .iter()
                    .position(|(handle, _)| *handle == listener)
                    .ok_or(TransportError::Closed)?;
                let (_, mut socket) = self.listeners.remove(index);
                let failure = socket.close().err().map(|_| Failure {
                    origin: FailureOrigin::Local,
                    code: FailureCode::ConnectionLost,
                });
                self.events
                    .push_back(Event::ListenerClosed { listener, failure });
                Ok(Accepted::Queued)
            }
            Request::OpenStream { connection, owner } => {
                let entry = self
                    .connections
                    .iter_mut()
                    .find(|entry| entry.handle() == &connection)
                    .ok_or(TransportError::WrongConnection)?;
                let stream = entry
                    .session()
                    .ok_or(TransportError::InvalidRequest)?
                    .open(&self.registry, &owner)
                    .map_err(session_error)?;
                Ok(Accepted::Stream(stream))
            }
            Request::Stream {
                owner,
                stream,
                action,
            } => {
                let entry = self
                    .connections
                    .iter_mut()
                    .find(|entry| entry.handle().id() == stream.connection_id())
                    .ok_or(TransportError::WrongConnection)?;
                let session = entry.session().ok_or(TransportError::InvalidRequest)?;
                match action {
                    StreamAction::Data(bytes) => session.send(&owner, &stream, bytes),
                    StreamAction::End => session.end(&owner, &stream),
                    StreamAction::Cancel(reason) => session.cancel(&owner, &stream, reason),
                }
                .map_err(session_error)?;
                Ok(Accepted::Queued)
            }
            Request::Ping {
                connection,
                token,
                timeout,
            } => {
                let now = self.dependencies.clock.now();
                self.connections
                    .iter_mut()
                    .find(|entry| entry.handle() == &connection)
                    .ok_or(TransportError::WrongConnection)?
                    .session()
                    .ok_or(TransportError::InvalidRequest)?
                    .ping(token, timeout, now)
                    .map_err(session_error)?;
                Ok(Accepted::Queued)
            }
            Request::Drain { connection, reason } => {
                self.connections
                    .iter_mut()
                    .find(|entry| entry.handle() == &connection)
                    .ok_or(TransportError::WrongConnection)?
                    .session()
                    .ok_or(TransportError::InvalidRequest)?
                    .drain(reason)
                    .map_err(session_error)?;
                Ok(Accepted::Queued)
            }
            Request::Close { connection } => {
                if let Some(index) = self
                    .pending
                    .iter()
                    .position(|entry| entry.handle == connection)
                {
                    self.pending.remove(index);
                    self.events.push_back(Event::ConnectionClosed {
                        connection: connection.id().clone(),
                        failure: Some(Failure {
                            origin: FailureOrigin::Local,
                            code: FailureCode::ConnectionLost,
                        }),
                    });
                } else {
                    self.connections
                        .iter_mut()
                        .find(|entry| entry.handle() == &connection)
                        .ok_or(TransportError::WrongConnection)?
                        .close();
                }
                Ok(Accepted::Queued)
            }
        }
    }

    // Admission consumes socket ownership; failure closes it before returning to composition.
    fn accept(
        &mut self,
        socket: Box<dyn Socket>,
        expected_peer: ExpectedPeer,
    ) -> Result<Accepted, TransportError> {
        tracing::trace!(
            function = "Driver::accept",
            "Admitting a caller-owned incoming socket"
        );
        if self.connections.len() + self.pending.len() >= self.config.limits().connections {
            return Err(TransportError::Backpressure);
        }
        let connection = Connection::begin_shared(
            socket,
            Options {
                role: ConnectionRole::Acceptor,
                expected_peer,
            },
            self.config,
            self.dependencies.clock.now(),
            &self.dependencies.identity,
            &mut self.dependencies.cryptography,
            self.resources.clone(),
        )
        .map_err(|d| {
            if d.failure.code == FailureCode::ResourceLimit {
                TransportError::Backpressure
            } else {
                TransportError::Startup(d.failure)
            }
        })?;
        let handle = connection.handle().clone();
        self.connections.push_back(connection);
        Ok(Accepted::Connection(handle))
    }
}
