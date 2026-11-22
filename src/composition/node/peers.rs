//! Bounded registry of authenticated live connections keyed by peer identity.
//!
//! Callers: the node loop records connects and closes; workers and policy read counts and
//! look up a live connection to a peer, whichever side opened it. That lets the node call a
//! client over the connection the client opened. The table holds only authenticated
//! connections, never exceeds [`MAXIMUM_LIVE`] entries and is cleared entry by entry as each
//! connection closes, so it cannot outlive a connection.
use crate::modules::{
    identity::api::IdentityId,
    transport::{
        api::{ConnectionRole, PeerContext},
        handles::{ConnectionHandle, ConnectionId},
    },
};
use std::sync::{Arc, Mutex};

/// Most live connections tracked; equal to the largest transport connection limit.
pub const MAXIMUM_LIVE: usize = 4096;

/// One authenticated live connection.
#[derive(Debug, Clone)]
pub struct Live {
    /// Authenticated identity of the remote peer.
    pub peer: IdentityId,
    /// Which side opened the connection.
    pub role: ConnectionRole,
    /// Local authority over the connection.
    pub connection: ConnectionHandle,
}

/// Shared registry; cloning shares the same table.
#[derive(Debug, Clone, Default)]
pub struct Peers {
    table: Arc<Mutex<Vec<Live>>>,
}

impl Peers {
    /// An empty registry.
    pub fn new() -> Self {
        tracing::trace!("Creating the live peer registry");
        Self::default()
    }

    /// Record an authenticated connection; returns false when the table is full or the connection is
    /// already recorded, in which case nothing changes.
    pub fn add(&self, context: &PeerContext, connection: ConnectionHandle) -> bool {
        tracing::trace!("Recording a live authenticated connection");
        let mut table = self.lock();
        if table.len() >= MAXIMUM_LIVE
            || table
                .iter()
                .any(|l| l.connection.id() == &context.connection)
        {
            return false;
        }
        table.push(Live {
            peer: context.peer_identity_id(),
            role: context.role,
            connection,
        });
        true
    }

    /// Forget a closed connection and return the peer it belonged to, if it was recorded.
    pub fn remove(&self, connection: &ConnectionId) -> Option<IdentityId> {
        tracing::trace!("Forgetting a closed connection");
        let mut table = self.lock();
        let index = table.iter().position(|l| l.connection.id() == connection)?;
        Some(table.remove(index).peer)
    }

    /// The peer a live connection belongs to.
    pub fn peer_of(&self, connection: &ConnectionId) -> Option<IdentityId> {
        tracing::trace!("Looking up the peer of a connection");
        self.lock()
            .iter()
            .find(|l| l.connection.id() == connection)
            .map(|l| l.peer)
    }

    /// Live connections in total.
    pub fn count(&self) -> usize {
        tracing::trace!("Counting live connections");
        self.lock().len()
    }

    /// Live connections held by one peer.
    pub fn count_for(&self, peer: IdentityId) -> usize {
        tracing::trace!("Counting one peer's live connections");
        self.lock().iter().filter(|l| l.peer == peer).count()
    }

    /// The newest live connection to `peer`, whichever side opened it.
    pub fn connection_to(&self, peer: IdentityId) -> Option<ConnectionHandle> {
        tracing::trace!("Looking up a live connection to a peer");
        self.lock()
            .iter()
            .rev()
            .find(|l| l.peer == peer)
            .map(|l| l.connection.clone())
    }

    /// Every live connection with its peer and local authority.
    pub fn live(&self) -> Vec<Live> {
        tracing::trace!("Listing live connections");
        self.lock().clone()
    }

    /// Distinct peers with at least one live connection, in first-connected order.
    pub fn peers(&self) -> Vec<IdentityId> {
        tracing::trace!("Listing connected peers");
        let mut seen = Vec::new();
        for live in self.lock().iter() {
            if !seen.contains(&live.peer) {
                seen.push(live.peer);
            }
        }
        seen
    }

    // A poisoned lock still holds a consistent table because every update is one push or remove.
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Live>> {
        self.table.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Conversion of the transport's identity type to the application's.
trait PeerIdentity {
    fn peer_identity_id(&self) -> IdentityId;
}
impl PeerIdentity for PeerContext {
    // Both identities are the same 32 bytes in different module types.
    fn peer_identity_id(&self) -> IdentityId {
        IdentityId(self.peer_identity.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::transport::ports::identity::IdentityId as WireId;

    // A context for a peer reached through `connection`.
    fn context(peer: u8, connection: &ConnectionHandle, role: ConnectionRole) -> PeerContext {
        PeerContext {
            connection: connection.id().clone(),
            role,
            local_identity: WireId([9; 32]),
            peer_identity: WireId([peer; 32]),
        }
    }

    // Connections are counted per peer and in total, found by peer and forgotten exactly when closed.
    #[test]
    fn connections_are_tracked_until_closed() {
        let peers = Peers::new();
        let (a1, a2, b) = (
            ConnectionHandle::new(),
            ConnectionHandle::new(),
            ConnectionHandle::new(),
        );
        assert!(peers.add(&context(1, &a1, ConnectionRole::Acceptor), a1.clone()));
        assert!(peers.add(&context(1, &a2, ConnectionRole::Initiator), a2.clone()));
        assert!(peers.add(&context(2, &b, ConnectionRole::Acceptor), b.clone()));
        assert_eq!(
            (peers.count(), peers.count_for(IdentityId([1; 32]))),
            (3, 2)
        );
        assert_eq!(
            peers.peers(),
            vec![IdentityId([1; 32]), IdentityId([2; 32])]
        );
        assert_eq!(peers.connection_to(IdentityId([1; 32])), Some(a2.clone()));
        assert_eq!(peers.peer_of(b.id()), Some(IdentityId([2; 32])));
        assert_eq!(peers.remove(a2.id()), Some(IdentityId([1; 32])));
        assert_eq!(peers.connection_to(IdentityId([1; 32])), Some(a1.clone()));
        assert_eq!(peers.remove(a2.id()), None);
        peers.remove(a1.id());
        peers.remove(b.id());
        assert_eq!(peers.count(), 0);
        assert!(peers.connection_to(IdentityId([1; 32])).is_none());
    }

    // Recording the same connection twice changes nothing, and the table never exceeds its bound.
    #[test]
    fn the_registry_is_bounded_and_idempotent() {
        let peers = Peers::new();
        let first = ConnectionHandle::new();
        let ctx = context(1, &first, ConnectionRole::Acceptor);
        assert!(peers.add(&ctx, first.clone()));
        assert!(!peers.add(&ctx, first.clone()));
        assert_eq!(peers.count(), 1);
        for _ in 1..MAXIMUM_LIVE {
            let h = ConnectionHandle::new();
            assert!(peers.add(&context(2, &h, ConnectionRole::Acceptor), h));
        }
        let extra = ConnectionHandle::new();
        assert!(!peers.add(&context(3, &extra, ConnectionRole::Acceptor), extra));
        assert_eq!(peers.count(), MAXIMUM_LIVE);
    }

    // Clones share one table, so the loop's writes are visible to workers.
    #[test]
    fn clones_share_the_table() {
        let peers = Peers::new();
        let view = peers.clone();
        let h = ConnectionHandle::new();
        peers.add(&context(1, &h, ConnectionRole::Acceptor), h.clone());
        assert_eq!(view.count_for(IdentityId([1; 32])), 1);
    }
}
