//! Thread-safe channel from local control workers to the single-threaded node loop.
//!
//! Callers: control command handlers. The loop owns Transport and the Peer Protocol
//! bridge, so workers never touch them: they send a [`Command`] and block on a bounded
//! reply. Every wait has a deadline, so a worker cannot hang the node on a lost reply.
use crate::{
    composition::server::pump::Signal,
    modules::{
        identity::api::IdentityId,
        peer_protocol::{engine::bridge::Factory, engine::endpoint::cache::PeerCache},
        transport::handles::ConnectionHandle,
    },
};
use std::{
    collections::BTreeMap,
    net::SocketAddr,
    sync::{
        Arc, Mutex,
        mpsc::{Sender, SyncSender, channel, sync_channel},
    },
    time::Duration,
};

/// A request a worker asks the node loop to perform.
pub enum Command {
    /// Open an outgoing connection pinned to the expected peer; the reply arrives after authentication.
    Connect {
        /// Remote endpoint.
        address: SocketAddr,
        /// Identity the handshake must prove.
        peer: IdentityId,
        /// Receives the authenticated connection or a failure description.
        reply: SyncSender<Result<ConnectionHandle, String>>,
    },
    /// Open one locally authorized exchange on an authenticated connection.
    Open {
        /// Authenticated connection to use.
        connection: ConnectionHandle,
        /// One consumed local intent bound to its completion channel.
        factory: Arc<dyn Factory>,
        /// Receives the admission result; the exchange outcome arrives on the plan's own channel.
        reply: SyncSender<Result<(), String>>,
    },
}

/// One authenticated outgoing connection and the direct metadata cache fetched over it.
#[derive(Clone)]
pub struct Session {
    /// Local authority over the connection instance.
    pub connection: ConnectionHandle,
    /// Direct-publisher cache for this peer's Board and definition; `None` until fetched.
    pub cache: Option<Arc<PeerCache>>,
}

/// Worker-side handle: command queue, loop wake-up and the reusable sessions per peer.
#[derive(Clone)]
pub struct Link {
    commands: Sender<Command>,
    signal: Signal,
    sessions: Arc<Mutex<BTreeMap<IdentityId, Session>>>,
}

impl Link {
    /// Create the link and the receiver the node loop drains.
    pub fn new(signal: Signal) -> (Self, std::sync::mpsc::Receiver<Command>) {
        tracing::trace!("Creating the node command link");
        let (commands, receiver) = channel();
        (
            Self {
                commands,
                signal,
                sessions: Arc::new(Mutex::new(BTreeMap::new())),
            },
            receiver,
        )
    }

    /// Return a live session for `peer`, connecting and authenticating when none exists.
    /// # Errors
    /// The node is stopping, the connection fails or does not authenticate within `timeout`.
    pub fn connect(
        &self,
        peer: IdentityId,
        address: SocketAddr,
        timeout: Duration,
    ) -> Result<Session, String> {
        tracing::trace!("Resolving a session to a configured peer");
        if let Some(session) = self.session(peer) {
            return Ok(session);
        }
        let (reply, receiver) = sync_channel(1);
        self.send(Command::Connect {
            address,
            peer,
            reply,
        })?;
        let connection = receiver
            .recv_timeout(timeout)
            .map_err(|_| "connection did not authenticate in time".to_string())??;
        let session = Session {
            connection,
            cache: None,
        };
        self.sessions
            .lock()
            .map_err(|_| "session table is unavailable".to_string())?
            .insert(peer, session.clone());
        Ok(session)
    }

    /// Open an exchange for a prepared plan on `connection`.
    /// # Errors
    /// The node is stopping, the connection is gone or admission is refused.
    pub fn open(
        &self,
        connection: &ConnectionHandle,
        factory: Arc<dyn Factory>,
        timeout: Duration,
    ) -> Result<(), String> {
        tracing::trace!("Opening a local exchange through the node loop");
        let (reply, receiver) = sync_channel(1);
        self.send(Command::Open {
            connection: connection.clone(),
            factory,
            reply,
        })?;
        receiver
            .recv_timeout(timeout)
            .map_err(|_| "the node did not admit the exchange in time".to_string())?
    }

    /// Retain the metadata cache fetched for a peer's current session.
    pub fn retain_cache(&self, peer: IdentityId, cache: Arc<PeerCache>) {
        tracing::trace!("Retaining a peer metadata cache on its session");
        if let Ok(mut sessions) = self.sessions.lock()
            && let Some(session) = sessions.get_mut(&peer)
        {
            session.cache = Some(cache);
        }
    }

    /// Forget sessions whose connection closed; called only by the node loop.
    pub fn closed(&self, connection: &crate::modules::transport::handles::ConnectionId) {
        tracing::trace!("Dropping sessions of a closed connection");
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.retain(|_, session| session.connection.id() != connection);
        }
    }

    /// Forget one peer's session so the next request reconnects.
    pub fn forget(&self, peer: IdentityId) {
        tracing::trace!("Forgetting a peer session");
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.remove(&peer);
        }
    }

    // Read one stored session without holding the lock across a wait.
    fn session(&self, peer: IdentityId) -> Option<Session> {
        tracing::trace!("Reading a stored peer session");
        self.sessions.lock().ok()?.get(&peer).cloned()
    }

    // Queue a command and wake the loop; a stopped loop makes the receiver disappear.
    fn send(&self, command: Command) -> Result<(), String> {
        tracing::trace!("Queueing a node command");
        if self.signal.stopping() {
            return Err("the node is stopping".into());
        }
        self.commands
            .send(command)
            .map_err(|_| "the node is stopping".to_string())?;
        self.signal.notify();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // A command reaches the loop side and the reply it sends comes back to the waiting worker.
    #[test]
    fn open_is_answered_by_the_loop() {
        let signal = Signal::default();
        let (link, commands) = Link::new(signal.clone());
        let connection = ConnectionHandle::new();
        let thread = std::thread::spawn({
            let link = link.clone();
            let connection = connection.clone();
            move || {
                struct Never;
                impl Factory for Never {
                    fn open(
                        &self,
                        _: crate::modules::peer_protocol::engine::bridge::Scope,
                        _: crate::modules::peer_protocol::exchange::Direction,
                        _: &mut crate::modules::peer_protocol::engine::bridge::Output,
                    ) -> Result<
                        Box<dyn crate::modules::peer_protocol::engine::bridge::Handler>,
                        crate::modules::peer_protocol::Error,
                    > {
                        Err(crate::modules::peer_protocol::Error::Unavailable)
                    }
                }
                link.open(&connection, Arc::new(Never), Duration::from_secs(5))
            }
        });
        let Command::Open { reply, .. } = commands.recv_timeout(Duration::from_secs(5)).unwrap()
        else {
            panic!("expected an open command");
        };
        reply.send(Ok(())).unwrap();
        assert!(thread.join().unwrap().is_ok());
    }
    // A worker gets a prompt error, not a hang, when the loop never answers or is stopping.
    #[test]
    fn unanswered_and_stopping_requests_fail_promptly() {
        let signal = Signal::default();
        let (link, _commands) = Link::new(signal.clone());
        let peer = IdentityId([1; 32]);
        let address: SocketAddr = "127.0.0.1:9".parse().unwrap();
        assert!(
            link.connect(peer, address, Duration::from_millis(50))
                .is_err()
        );
        signal.shutdown();
        assert_eq!(
            link.connect(peer, address, Duration::from_millis(50))
                .err()
                .unwrap(),
            "the node is stopping"
        );
        let (link, commands) = Link::new(Signal::default());
        drop(commands);
        assert!(
            link.connect(peer, address, Duration::from_millis(50))
                .is_err()
        );
    }
    // Closing a connection forgets exactly its sessions.
    #[test]
    fn closed_connections_drop_their_sessions() {
        let (link, _commands) = Link::new(Signal::default());
        let connection = ConnectionHandle::new();
        link.sessions.lock().unwrap().insert(
            IdentityId([1; 32]),
            Session {
                connection: connection.clone(),
                cache: None,
            },
        );
        link.closed(connection.id());
        assert!(link.session(IdentityId([1; 32])).is_none());
    }
}
