//! Local request/event contract implemented by the composed Transport driver.
//!
//! Only trusted composition receives the control port and registers consumers.
//! Requests are local calls, never deserialized peer messages. The driver must
//! check input bounds, live handles, connection scope and caller ownership
//! before effects. Accepted/queued work is not proof of remote receipt.

use std::{
    net::SocketAddr,
    task::{Context, Poll},
    time::Duration,
};

use super::{
    TransportError,
    handles::{ConnectionHandle, ConnectionId, ListenerHandle, StreamHandle, StreamOwner},
    ports::{Dependency, identity::IdentityId, io::Socket},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionRole {
    Initiator,
    Acceptor,
}

/// Known contacts must select `Known`; unpinned discovery still requires AUTH.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedPeer {
    Known(IdentityId),
    Unpinned,
}

/// An outgoing endpoint cannot use the ephemeral-bind convention of port zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectRequest {
    endpoint: SocketAddr,
    expected_peer: ExpectedPeer,
}

impl ConnectRequest {
    pub fn new(endpoint: SocketAddr, expected_peer: ExpectedPeer) -> Result<Self, TransportError> {
        if endpoint.port() == 0 {
            return Err(TransportError::InvalidEndpoint);
        }
        Ok(Self {
            endpoint,
            expected_peer,
        })
    }

    pub fn endpoint(&self) -> SocketAddr {
        self.endpoint
    }

    pub fn expected_peer(&self) -> ExpectedPeer {
        self.expected_peer
    }
}

/// Requested protocol selection; the OPEN codec validates grammar and version.
/// Carrying this value does not prove registration or protocol support.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Protocol {
    pub id: String,
    pub version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancellationReason {
    Cancelled,
    ResourceLimit,
    InternalError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownReason {
    Normal,
    IdExhausted,
    ResourceLimit,
    InternalError,
}

#[derive(Debug)]
pub enum StreamAction {
    /// Ordered bytes subject to local queue limits; empty sends are invalid.
    Data(Vec<u8>),
    End,
    Cancel(CancellationReason),
}

/// Control port requests; identity and default limits come from composition.
pub enum Request {
    Connect(ConnectRequest),
    /// Port zero selects an OS-assigned port, unlike an outgoing connection.
    Listen {
        endpoint: SocketAddr,
    },
    /// Transfer socket ownership; no authentication is implied by acceptance.
    Accept {
        socket: Box<dyn Socket>,
        expected_peer: ExpectedPeer,
    },
    StopListening {
        listener: ListenerHandle,
    },
    /// Success returns an owner capability for this protocol's streams.
    RegisterProtocol {
        protocol: Protocol,
    },
    /// The owner selects its registered protocol, not a peer-supplied handler.
    OpenStream {
        connection: ConnectionHandle,
        owner: StreamOwner,
    },
    Stream {
        owner: StreamOwner,
        stream: StreamHandle,
        action: StreamAction,
    },
    /// Timeout must be positive and tokens must distinguish outstanding pings.
    Ping {
        connection: ConnectionHandle,
        token: [u8; 8],
        timeout: Duration,
    },
    Drain {
        connection: ConnectionHandle,
        reason: ShutdownReason,
    },
    Close {
        connection: ConnectionHandle,
    },
}

/// Immediate local acceptance; network outcomes arrive as events.
#[derive(Debug)]
pub enum Accepted {
    /// A pending handshake; this does not authenticate the peer.
    Connection(ConnectionHandle),
    Listener {
        handle: ListenerHandle,
        local_addr: SocketAddr,
    },
    Registration(StreamOwner),
    Stream(StreamHandle),
    /// An accepted control/data request, not a remote acknowledgement.
    Queued,
}

/// Public observations emitted only after successful handshake authentication.
/// Observation contains no capability to close a connection or sign messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerContext {
    pub connection: ConnectionId,
    pub role: ConnectionRole,
    pub local_identity: IdentityId,
    pub peer_identity: IdentityId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureOrigin {
    Local,
    Peer,
    Dependency(Dependency),
}

/// Logical outcomes, not numeric wire encodings or diagnostic-text decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureCode {
    ProtocolViolation,
    AuthenticationFailed,
    IntegrityFailed,
    FrameTooLarge,
    UnsupportedProtocol,
    StreamState,
    Refused,
    /// Explicit peer cancellation used in an ERROR payload.
    Cancelled,
    ResourceLimit,
    IdExhausted,
    SequenceExhausted,
    TruncatedFrame,
    ConnectionLost,
    DeadlineExceeded,
    DependencyUnavailable,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Failure {
    pub origin: FailureOrigin,
    pub code: FailureCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamOutcome {
    /// Permitted only after END in both directions.
    Completed,
    Cancelled {
        origin: FailureOrigin,
        reason: CancellationReason,
    },
    Failed(Failure),
}

/// Events are queued for trusted composition to route to the owning consumer.
/// Never call application handlers while holding internal connection locks.
#[derive(Debug)]
pub enum Event {
    /// Gives the local controller authority over a newly accepted socket.
    /// It carries no authenticated peer context yet.
    IncomingConnection(ConnectionHandle),
    Authenticated(PeerContext),
    StreamOpened {
        context: PeerContext,
        stream: StreamHandle,
        protocol: Protocol,
    },
    /// Verified bytes only; DATA frame boundaries have no application meaning.
    Data {
        stream: StreamHandle,
        bytes: Vec<u8>,
    },
    RemoteEnd {
        stream: StreamHandle,
    },
    StreamClosed {
        stream: StreamHandle,
        outcome: StreamOutcome,
    },
    PingFinished {
        connection: ConnectionId,
        token: [u8; 8],
        result: Result<Duration, Failure>,
    },
    /// None means a normal drain; Some preserves the cause of failed work.
    ConnectionClosed {
        connection: ConnectionId,
        failure: Option<Failure>,
    },
    ListenerClosed {
        listener: ListenerHandle,
        failure: Option<Failure>,
    },
}

/// Control interface implemented by driver::Driver with explicitly supplied dependencies.
/// One owner submits requests and polls events, while adapters wake that owner.
/// Consumers can run elsewhere through bounded message queues.
pub trait TransportControl {
    /// Validate input/authority before admission; errors queue no work.
    fn submit(&mut self, request: Request) -> Result<Accepted, TransportError>;
    /// Pending arranges a wakeup; None means the driver has stopped.
    fn poll_event(&mut self, cx: &mut Context<'_>) -> Poll<Option<Event>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outgoing_port_zero_is_rejected_without_io() {
        let endpoint = "127.0.0.1:0".parse().unwrap();
        assert_eq!(
            ConnectRequest::new(endpoint, ExpectedPeer::Unpinned),
            Err(TransportError::InvalidEndpoint)
        );
    }

    #[test]
    fn known_contact_keeps_its_expected_identity() {
        let endpoint = "[::1]:9000".parse().unwrap();
        let expected = ExpectedPeer::Known(IdentityId([7; 32]));
        let request = ConnectRequest::new(endpoint, expected).unwrap();
        assert_eq!(request.endpoint(), endpoint);
        assert_eq!(request.expected_peer(), expected);
    }

    #[test]
    fn unpinned_connection_is_explicit_and_retained() {
        let request =
            ConnectRequest::new("127.0.0.1:9000".parse().unwrap(), ExpectedPeer::Unpinned).unwrap();
        assert_eq!(request.expected_peer(), ExpectedPeer::Unpinned);
    }
}
