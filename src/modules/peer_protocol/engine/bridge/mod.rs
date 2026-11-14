//! Authenticated Transport stream scheduling with bounded asynchronous dependency ownership.
//! Composition owns the Transport control port and installs an explicit trusted factory.
//! This adapter owns framing and delivery observations; handlers own exchange semantics.
use super::worker::{Pool, Ticket};
use crate::modules::peer_protocol::{
    Error,
    contract::{Association, OperationKey},
    exchange::Direction,
    outbound::Outbound,
    record::{CONTROL_MAX, Record},
};
use crate::modules::{
    identity::api::IdentityId,
    transport::{
        TransportError,
        api::{
            Accepted, CancellationReason, Event, PeerContext, Protocol, Request, StreamAction,
            StreamOutcome, TransportControl,
        },
        handles::{ConnectionHandle, StreamHandle, StreamOwner},
    },
};
use std::{
    collections::VecDeque,
    sync::Arc,
    task::{Context, Poll},
    time::{Duration, Instant},
};

mod contract;
mod events;
mod execution;
mod scheduling;
pub use contract::{Closure, Config, Factory, Handler, Output, Scope};
use execution::{Input, Returned, State};
struct Connection {
    context: PeerContext,
    nonce: [u8; 32],
}
struct Entry {
    id: u64,
    stream: StreamHandle,
    opened: bool,
    state: Option<State>,
    flight: Option<Ticket>,
    pending: VecDeque<Input>,
    input_bytes: usize,
    closed: Option<Closure>,
    closing: bool,
    flight_bytes: usize,
    termination: Option<Stop>,
    end_sent: bool,
    deadline: Instant,
}
/// Single-reactor adapter over the actual Transport control interface.
///
/// Call `event` for every Transport observation and `poll` on every reactor turn,
/// including turns where Transport made write progress without emitting an event.
/// Route returned lifecycle/foreign-protocol events to other composition owners.
/// Factory callbacks execute only on fixed workers. Byte reservations cover adapter
/// buffers, not arbitrary application state: handlers must enforce their own budgets.
/// This type is opt-in and does not modify the default Transport-only server.
pub struct Bridge {
    owner: StreamOwner,
    config: Config,
    reservation: usize,
    connections: Vec<Connection>,
    entries: Vec<Entry>,
    pool: Pool<Returned>,
    factory: Arc<dyn Factory>,
    domain: [u8; 24],
    connection_sequence: u64,
    stream_sequence: u64,
    stopped: bool,
}
impl Bridge {
    /// Register `cr2se.peer` version1 with an explicitly installed, complete trusted factory.
    /// A fresh OS-random process domain and nonwrapping counter identify connection instances.
    /// # Errors
    /// Invalid limits, failed entropy/thread creation or rejected registration fail startup.
    /// Callers must not install test substitutes or advertise unsupported handler capabilities.
    pub fn register(
        driver: &mut impl TransportControl,
        config: Config,
        factory: Arc<dyn Factory>,
    ) -> Result<Self, Error> {
        tracing::trace!("Registering an explicit authenticated Peer Protocol consumer");
        let reservation = config.reservation()?;
        let mut domain = [0; 24];
        getrandom::fill(&mut domain).map_err(|_| Error::Unavailable)?;
        let pool = Pool::new(config.workers, config.exchanges)?;
        let Accepted::Registration(owner) = driver
            .submit(Request::RegisterProtocol {
                protocol: protocol(),
            })
            .map_err(map_transport)?
        else {
            return Err(Error::Unavailable);
        };
        Ok(Self {
            owner,
            config,
            reservation,
            connections: Vec::new(),
            entries: Vec::new(),
            pool,
            factory,
            domain,
            connection_sequence: 0,
            stream_sequence: 0,
            stopped: false,
        })
    }
    /// Open a stream only on a previously authenticated connection instance.
    /// # Errors
    /// Missing authority, stopped admission, bounds or Transport refusal has no application success.
    pub fn open(
        &mut self,
        driver: &mut impl TransportControl,
        connection: &ConnectionHandle,
    ) -> Result<StreamHandle, Error> {
        tracing::trace!("Opening a peer exchange on a verified connection instance");
        self.open_with(driver, connection, self.factory.clone())
    }
    /// Open one locally requested exchange using an explicit trusted per-call factory.
    /// The override receives the same verified scope and budgets; peer data cannot install it.
    /// # Errors
    /// Authentication, admission or Transport refusal queues no application success.
    pub fn open_with(
        &mut self,
        driver: &mut impl TransportControl,
        connection: &ConnectionHandle,
        factory: Arc<dyn Factory>,
    ) -> Result<StreamHandle, Error> {
        tracing::trace!("Opening one explicit locally authorized peer request");
        self.capacity()?;
        let context = self
            .connections
            .iter()
            .find(|c| &c.context.connection == connection.id())
            .ok_or(Error::Denied)?
            .context
            .clone();
        let Accepted::Stream(stream) = driver
            .submit(Request::OpenStream {
                connection: connection.clone(),
                owner: self.owner.clone(),
            })
            .map_err(map_transport)?
        else {
            return Err(Error::Unavailable);
        };
        if let Err(error) = self.admit(context, stream.clone(), Direction::Local, factory) {
            cancel(
                driver,
                &self.owner,
                &stream,
                StreamAction::Cancel(CancellationReason::InternalError),
            );
            return Err(error);
        }
        Ok(stream)
    }
    /// Refuse new streams while continuing accepted work, output and closure cleanup.
    pub fn stop(&mut self) {
        tracing::trace!("Stopping new peer exchanges while retaining accepted owners");
        self.stopped = true;
    }
    /// Count live streams, including accepted work still awaiting a late completion or cleanup.
    pub fn active(&self) -> usize {
        tracing::trace!("Inspecting active peer adapter exchanges");
        self.entries.len()
    }
    // Check conservative aggregate reservations before allocating another per-stream decoder or queue.
    fn capacity(&self) -> Result<(), Error> {
        tracing::trace!("Checking finite peer stream admission");
        if Instant::now().checked_add(self.config.lifetime).is_none() {
            return Err(Error::Invalid);
        }
        if self.stopped
            || self.stream_sequence == u64::MAX
            || self.entries.len() >= self.config.exchanges
            || self.entries.len() >= self.config.memory_bytes / self.reservation
        {
            return Err(Error::Limit);
        }
        Ok(())
    }
    // Authorize original stream/connection context before associating peer identities with application calls.
    fn admit(
        &mut self,
        context: PeerContext,
        stream: StreamHandle,
        opener: Direction,
        factory: Arc<dyn Factory>,
    ) -> Result<(), Error> {
        tracing::trace!("Admitting one authenticated original peer stream");
        self.capacity()?;
        if stream.authorize(&context.connection, &self.owner).is_err()
            || self.entries.iter().any(|e| e.stream == stream)
        {
            return Err(Error::Denied);
        }
        let connection = self
            .connections
            .iter()
            .find(|c| c.context == context)
            .ok_or(Error::Denied)?;
        self.stream_sequence = self.stream_sequence.checked_add(1).ok_or(Error::Limit)?;
        let local = IdentityId(context.local_identity.0);
        let peer = IdentityId(context.peer_identity.0);
        let deadline = Instant::now()
            .checked_add(self.config.lifetime)
            .ok_or(Error::Invalid)?;
        let scope = Scope {
            local,
            peer,
            connection: connection.nonce,
            stream: stream.wire_id().get(),
            deadline,
        };
        let state = State::new(factory, scope, opener, self.config)?;
        self.entries.push(Entry {
            id: self.stream_sequence,
            stream,
            opened: opener == Direction::Remote,
            state: Some(state),
            flight: None,
            pending: VecDeque::from([Input::Open]),
            input_bytes: 0,
            closed: None,
            closing: false,
            flight_bytes: 0,
            termination: None,
            end_sent: false,
            deadline,
        });
        Ok(())
    }
    // Ownership comes from Transport's opaque capability, never a matching numeric stream ID alone.
    fn owns(&self, stream: &StreamHandle) -> bool {
        tracing::trace!("Checking opaque peer stream ownership");
        stream
            .authorize(stream.connection_id(), &self.owner)
            .is_ok()
    }
}
// Select the specified application protocol; optional service families are independent installations.
fn protocol() -> Protocol {
    tracing::trace!("Selecting the Peer Protocol wire version");
    Protocol {
        id: "cr2se.peer".into(),
        version: 1,
    }
}
// Preserve known local queue refusal versus unknown dependency or runtime outcomes.
fn map_transport(error: TransportError) -> Error {
    tracing::trace!("Mapping Transport admission at the peer adapter boundary");
    match error {
        TransportError::Backpressure => Error::Limit,
        TransportError::WrongConnection | TransportError::WrongStreamOwner => Error::Denied,
        _ => Error::Unavailable,
    }
}
// Request only owner-scoped stream termination; no accepted work is undone by this request.
fn cancel(
    driver: &mut impl TransportControl,
    owner: &StreamOwner,
    stream: &StreamHandle,
    action: StreamAction,
) {
    tracing::trace!("Terminating an affected peer stream without closing unrelated streams");
    let _ = driver.submit(Request::Stream {
        owner: owner.clone(),
        stream: stream.clone(),
        action,
    });
}
// Preserve the first actual closure and discard input that can no longer authorize fresh effects.
fn close(entry: &mut Entry, outcome: StreamOutcome) {
    tracing::trace!("Retaining Transport closure for asynchronous peer cleanup");
    if entry.closed.is_some() {
        return;
    }
    entry.closed = Some(Closure {
        outcome,
        received_at: Instant::now(),
        local_end_submitted: entry.end_sent,
    });
    if !matches!(outcome, StreamOutcome::Completed) {
        entry.pending.clear();
        entry.input_bytes = entry.flight_bytes;
    }
}
/// Internal termination intent survives Transport backpressure without changing its wire code.
#[derive(Clone, Copy)]
enum Stop {
    Protocol,
    Resource,
    Cancelled,
    Internal,
}
impl Stop {
    // Reconstruct a fresh owner-scoped action for safe retry after refused local admission only.
    fn action(self) -> StreamAction {
        tracing::trace!("Constructing a pending peer stream termination action");
        match self {
            Self::Protocol => StreamAction::ProtocolError,
            Self::Resource => StreamAction::Cancel(CancellationReason::ResourceLimit),
            Self::Cancelled => StreamAction::Cancel(CancellationReason::Cancelled),
            Self::Internal => StreamAction::Cancel(CancellationReason::InternalError),
        }
    }
}
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(in crate::modules::peer_protocol) mod test_support;
