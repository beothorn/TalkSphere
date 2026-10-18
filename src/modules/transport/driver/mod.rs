//! Trusted Transport control composition with shared dependencies, budgets and queued outcomes.
use super::{
    TransportError,
    api::*,
    config::TransportConfig,
    connection::{Connection, Options},
    handles::{ConnectionHandle, ListenerHandle},
    ports::{
        Dependencies,
        crypto::Cryptography,
        identity::Identity,
        io::{Clock, Connecting, Listener, SocketProvider},
    },
    resources::{Reservation, Resources},
    stream::{registry::Registry, session::SessionError},
};
use std::{
    collections::VecDeque,
    task::{Context, Poll, Waker},
    time::Instant,
};
mod requests;

/// Owns all connections/listeners for one local Identity capability and finite policy.
pub struct Driver<S: SocketProvider, T: Clock, I: Identity, C: Cryptography> {
    dependencies: Dependencies<S, T, I, C>,
    config: TransportConfig,
    resources: Resources,
    registry: Registry,
    registrations: usize,
    connections: VecDeque<Connection<C>>,
    pending: Vec<Pending>,
    listeners: Vec<(ListenerHandle, Box<dyn Listener>)>,
    events: VecDeque<Event>,
    waker: Option<Waker>,
}
struct Pending {
    handle: ConnectionHandle,
    attempt: Box<dyn Connecting>,
    expected: ExpectedPeer,
    deadline: Instant,
    reservation: Reservation,
}

impl<S: SocketProvider, T: Clock, I: Identity, C: Cryptography> Driver<S, T, I, C> {
    /// Compose validated policy and real or test-owned dependency capabilities without binding sockets.
    pub fn new(config: TransportConfig, dependencies: Dependencies<S, T, I, C>) -> Self {
        tracing::trace!(
            function = "Driver::new",
            "Composing a bounded Transport instance"
        );
        Self {
            dependencies,
            config,
            resources: Resources::new(*config.limits()),
            registry: Registry::default(),
            registrations: 0,
            connections: VecDeque::new(),
            pending: Vec::new(),
            listeners: Vec::new(),
            events: VecDeque::new(),
            waker: None,
        }
    }
    /// Inspect shared occupancy without obtaining socket or signing authority.
    pub fn resources(&self) -> &Resources {
        tracing::trace!(
            function = "Driver::resources",
            "Reading Transport resource occupancy"
        );
        &self.resources
    }

    // Complete nonblocking outgoing attempts, preserving their originally returned handles.
    fn poll_connects(&mut self, cx: &mut Context<'_>, now: Instant) {
        tracing::trace!(
            function = "Driver::poll_connects",
            "Polling admitted outgoing connections"
        );
        let mut index = 0;
        while index < self.pending.len() {
            let result = if now >= self.pending[index].deadline {
                Poll::Ready(Err(std::io::ErrorKind::TimedOut.into()))
            } else {
                self.pending[index].attempt.poll_connected(cx)
            };
            let Poll::Ready(result) = result else {
                index += 1;
                continue;
            };
            let pending = self.pending.remove(index);
            drop(pending.reservation);
            let result = result
                .map_err(|error| Failure {
                    origin: FailureOrigin::Local,
                    code: if error.kind() == std::io::ErrorKind::TimedOut {
                        FailureCode::DeadlineExceeded
                    } else {
                        FailureCode::ConnectionLost
                    },
                })
                .and_then(|socket| {
                    Connection::begin_shared(
                        socket,
                        Options {
                            role: ConnectionRole::Initiator,
                            expected_peer: pending.expected,
                        },
                        self.config,
                        now,
                        &self.dependencies.identity,
                        &mut self.dependencies.cryptography,
                        self.resources.clone(),
                    )
                    .map_err(|d| d.failure)
                });
            match result {
                Ok(mut connection) => {
                    connection.set_handle(pending.handle);
                    self.connections.push_back(connection);
                }
                Err(failure) => self.events.push_back(Event::ConnectionClosed {
                    connection: pending.handle.id().clone(),
                    failure: Some(failure),
                }),
            }
        }
    }

    // Admit at most one ready socket per listener per poll; no credentials are allocated beyond capacity.
    fn poll_listeners(&mut self, cx: &mut Context<'_>, now: Instant) {
        tracing::trace!(
            function = "Driver::poll_listeners",
            "Polling bounded incoming connection admission"
        );
        let mut index = 0;
        while index < self.listeners.len() {
            if self.resources.connections() >= self.config.limits().connections
                || self.connections.len() + self.pending.len() >= self.config.limits().connections
            {
                break;
            }
            match self.listeners[index].1.poll_accept(cx) {
                Poll::Pending => index += 1,
                Poll::Ready(Err(_)) => {
                    let (listener, mut socket) = self.listeners.remove(index);
                    let _ = socket.close();
                    self.events.push_back(Event::ListenerClosed {
                        listener,
                        failure: Some(Failure {
                            origin: FailureOrigin::Local,
                            code: FailureCode::ConnectionLost,
                        }),
                    });
                }
                Poll::Ready(Ok(socket)) => {
                    match Connection::begin_shared(
                        socket,
                        Options {
                            role: ConnectionRole::Acceptor,
                            expected_peer: ExpectedPeer::Unpinned,
                        },
                        self.config,
                        now,
                        &self.dependencies.identity,
                        &mut self.dependencies.cryptography,
                        self.resources.clone(),
                    ) {
                        Ok(connection) => {
                            self.events
                                .push_back(Event::IncomingConnection(connection.handle().clone()));
                            self.connections.push_back(connection);
                        }
                        Err(disposition) => {
                            let handle = ConnectionHandle::new();
                            self.events.push_back(Event::ConnectionClosed {
                                connection: handle.id().clone(),
                                failure: Some(disposition.failure),
                            });
                        }
                    }
                    index += 1;
                    cx.waker().wake_by_ref();
                }
            }
        }
    }

    // Register just the earliest timer because the Clock contract replaces its previous deadline.
    fn arm_timer(&mut self, cx: &mut Context<'_>) {
        tracing::trace!(
            function = "Driver::arm_timer",
            "Arming the earliest Transport deadline"
        );
        let deadline = self
            .pending
            .iter()
            .map(|p| p.deadline)
            .chain(
                self.connections
                    .iter()
                    .filter_map(Connection::next_deadline),
            )
            .min();
        if let Some(deadline) = deadline
            && self
                .dependencies
                .clock
                .poll_deadline(cx, deadline)
                .is_ready()
        {
            cx.waker().wake_by_ref();
        }
    }
}

impl<S: SocketProvider, T: Clock, I: Identity, C: Cryptography> TransportControl
    for Driver<S, T, I, C>
{
    /// Validate local authority and finite admission before accepting caller work.
    fn submit(&mut self, request: Request) -> Result<Accepted, TransportError> {
        tracing::trace!(
            function = "Driver::submit",
            "Submitting an authorized Transport operation"
        );
        if self.events.len()
            >= self
                .config
                .limits()
                .connections
                .saturating_mul(4)
                .saturating_add(16)
        {
            return Err(TransportError::Backpressure);
        }
        let result = self.request(request);
        if result.is_ok()
            && let Some(waker) = self.waker.take()
        {
            waker.wake();
        }
        result
    }
    /// Poll fair connection work and transfer one queued observation to trusted composition.
    fn poll_event(&mut self, cx: &mut Context<'_>) -> Poll<Option<Event>> {
        tracing::trace!(
            function = "Driver::poll_event",
            "Polling Transport observations"
        );
        self.waker = Some(cx.waker().clone());
        if let Some(event) = self.events.pop_front() {
            return Poll::Ready(Some(event));
        }
        let now = self.dependencies.clock.now();
        self.poll_connects(cx, now);
        self.poll_listeners(cx, now);
        if let Some(event) = self.events.pop_front() {
            self.arm_timer(cx);
            return Poll::Ready(Some(event));
        }
        for _ in 0..self.connections.len() {
            let Some(mut connection) = self.connections.pop_front() else {
                break;
            };
            let closed = connection
                .poll(
                    cx,
                    now,
                    &self.registry,
                    &mut self.dependencies.identity,
                    &mut self.dependencies.cryptography,
                )
                .is_ready();
            let event = connection.pop_event();
            if !closed || event.is_some() {
                self.connections.push_back(connection);
            } else {
                cx.waker().wake_by_ref();
            }
            if let Some(event) = event {
                self.arm_timer(cx);
                return Poll::Ready(Some(event));
            }
        }
        self.arm_timer(cx);
        match self.events.pop_front() {
            Some(event) => Poll::Ready(Some(event)),
            None => Poll::Pending,
        }
    }
}

// Translate immediate local rejections without erasing capability or backpressure distinctions.
fn session_error(error: SessionError) -> TransportError {
    tracing::trace!(
        function = "driver::session_error",
        "Mapping local session admission failure"
    );
    match error {
        SessionError::Capacity => TransportError::Backpressure,
        SessionError::Authority(error) => error,
        SessionError::Owner => TransportError::WrongStreamOwner,
        SessionError::Closed => TransportError::Closed,
        _ => TransportError::InvalidRequest,
    }
}

#[cfg(test)]
mod tests;
