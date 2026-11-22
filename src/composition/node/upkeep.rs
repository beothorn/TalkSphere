//! Connection upkeep for the client profile: reconnect to configured servers and keep live
//! connections healthy with pings.
//!
//! Callers: the node loop asks what is due, performs the Transport requests itself and reports
//! results back. This file holds only the schedule so it can be tested without sockets: a
//! reconnect state per configured contact with bounded exponential delay from the policy, and
//! one outstanding ping per live connection with the policy's interval and timeout. Nothing
//! here opens a connection or blocks.
use crate::modules::{identity::api::IdentityId, transport::handles::ConnectionId};
use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};

/// A configured server this node keeps a connection to.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Target {
    peer: IdentityId,
    endpoint: SocketAddr,
    /// Failed or lost attempts since the last success.
    attempts: u32,
    /// Earliest time of the next attempt.
    next_at: Instant,
    /// Whether an attempt is in flight.
    pending: bool,
}

/// What the loop must do now for one live connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PingDue {
    /// The connection to ping.
    pub connection: ConnectionId,
    /// Token that identifies this ping's answer.
    pub token: [u8; 8],
}

#[derive(Debug, Clone)]
struct Health {
    connection: ConnectionId,
    last_sent: Instant,
    outstanding: Option<[u8; 8]>,
}

/// Reconnect and ping schedule.
#[derive(Debug)]
pub struct Upkeep {
    targets: Vec<Target>,
    health: Vec<Health>,
    next_token: u64,
}

impl Upkeep {
    /// Track `contacts`, each due immediately.
    pub fn new(contacts: &[(IdentityId, SocketAddr)], now: Instant) -> Self {
        tracing::trace!("Creating the connection upkeep schedule");
        Self {
            targets: contacts
                .iter()
                .map(|(peer, endpoint)| Target {
                    peer: *peer,
                    endpoint: *endpoint,
                    attempts: 0,
                    next_at: now,
                    pending: false,
                })
                .collect(),
            health: Vec::new(),
            next_token: 0,
        }
    }

    /// Contacts to connect to now: not connected (`connected` says whether a peer has a live
    /// connection), no attempt in flight and past their delay. Each returned contact is marked in flight.
    pub fn due_connects(
        &mut self,
        now: Instant,
        connected: impl Fn(IdentityId) -> bool,
    ) -> Vec<(IdentityId, SocketAddr)> {
        tracing::trace!("Finding contacts due for a connection attempt");
        let mut due = Vec::new();
        for target in &mut self.targets {
            if !target.pending && now >= target.next_at && !connected(target.peer) {
                target.pending = true;
                due.push((target.peer, target.endpoint));
            }
        }
        due
    }

    /// An attempt authenticated: the next loss starts again from the shortest delay.
    pub fn connected(&mut self, peer: IdentityId) {
        tracing::trace!("Recording a successful contact connection");
        for target in self.targets.iter_mut().filter(|t| t.peer == peer) {
            target.attempts = 0;
            target.pending = false;
        }
    }

    /// An attempt failed or a connection to a contact was lost; retry after `delay`.
    pub fn failed(&mut self, peer: IdentityId, now: Instant, delay: Duration) {
        tracing::trace!("Scheduling a contact reconnect");
        for target in self.targets.iter_mut().filter(|t| t.peer == peer) {
            target.attempts = target.attempts.saturating_add(1);
            target.pending = false;
            target.next_at = now + delay;
        }
    }

    /// Failed attempts since the last success, for choosing the next delay.
    pub fn attempts(&self, peer: IdentityId) -> u32 {
        tracing::trace!("Reading the contact attempt count");
        self.targets
            .iter()
            .find(|t| t.peer == peer)
            .map_or(0, |t| t.attempts)
    }

    /// Whether `peer` is a configured contact.
    pub fn is_contact(&self, peer: IdentityId) -> bool {
        tracing::trace!("Checking for a configured contact");
        self.targets.iter().any(|t| t.peer == peer)
    }

    /// Pings due now among `live` connections: those with none outstanding and at least `interval`
    /// since the last one. Connections not in `live` are forgotten.
    pub fn due_pings(
        &mut self,
        now: Instant,
        interval: Duration,
        live: &[ConnectionId],
    ) -> Vec<PingDue> {
        tracing::trace!("Finding connections due for a ping");
        self.health.retain(|h| live.contains(&h.connection));
        for connection in live {
            if !self.health.iter().any(|h| &h.connection == connection) {
                self.health.push(Health {
                    connection: connection.clone(),
                    last_sent: now,
                    outstanding: None,
                });
            }
        }
        let mut due = Vec::new();
        for health in &mut self.health {
            if health.outstanding.is_none() && now.duration_since(health.last_sent) >= interval {
                self.next_token = self.next_token.wrapping_add(1);
                let token = self.next_token.to_be_bytes();
                health.outstanding = Some(token);
                health.last_sent = now;
                due.push(PingDue {
                    connection: health.connection.clone(),
                    token,
                });
            }
        }
        due
    }

    /// A ping finished; returns whether it was one this schedule sent, so a stray answer is ignored.
    pub fn ping_finished(&mut self, connection: &ConnectionId, token: [u8; 8]) -> bool {
        tracing::trace!("Recording a finished ping");
        match self
            .health
            .iter_mut()
            .find(|h| &h.connection == connection && h.outstanding == Some(token))
        {
            Some(health) => {
                health.outstanding = None;
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::transport::handles::ConnectionHandle;

    // A distinct identity per byte and a fixed endpoint.
    fn peer(n: u8) -> IdentityId {
        IdentityId([n; 32])
    }
    fn endpoint() -> SocketAddr {
        "127.0.0.1:9".parse().unwrap()
    }

    // A contact is due at once, only once while its attempt is in flight, and not while connected.
    #[test]
    fn contacts_connect_once_until_resolved() {
        let now = Instant::now();
        let mut upkeep = Upkeep::new(&[(peer(1), endpoint())], now);
        assert_eq!(
            upkeep.due_connects(now, |_| false),
            vec![(peer(1), endpoint())]
        );
        assert!(upkeep.due_connects(now, |_| false).is_empty());
        upkeep.connected(peer(1));
        assert!(upkeep.due_connects(now, |_| true).is_empty());
        assert!(upkeep.is_contact(peer(1)));
        assert!(!upkeep.is_contact(peer(2)));
    }

    // After a failure the contact waits for the delay, the attempt count grows and success resets it.
    #[test]
    fn failures_back_off_and_success_resets() {
        let now = Instant::now();
        let mut upkeep = Upkeep::new(&[(peer(1), endpoint())], now);
        upkeep.due_connects(now, |_| false);
        upkeep.failed(peer(1), now, Duration::from_secs(4));
        assert_eq!(upkeep.attempts(peer(1)), 1);
        assert!(
            upkeep
                .due_connects(now + Duration::from_secs(3), |_| false)
                .is_empty()
        );
        assert_eq!(
            upkeep
                .due_connects(now + Duration::from_secs(4), |_| false)
                .len(),
            1
        );
        upkeep.failed(peer(1), now, Duration::from_secs(8));
        assert_eq!(upkeep.attempts(peer(1)), 2);
        upkeep.connected(peer(1));
        assert_eq!(upkeep.attempts(peer(1)), 0);
    }

    // A contact that is connected by another route (the server connected to it) is never dialed.
    #[test]
    fn an_existing_connection_suppresses_dialing() {
        let now = Instant::now();
        let mut upkeep = Upkeep::new(&[(peer(1), endpoint()), (peer(2), endpoint())], now);
        let due = upkeep.due_connects(now, |p| p == peer(1));
        assert_eq!(due, vec![(peer(2), endpoint())]);
    }

    // Pings start one interval after a connection appears, never overlap, and a stray answer is ignored.
    #[test]
    fn pings_follow_the_interval_without_overlap() {
        let start = Instant::now();
        let mut upkeep = Upkeep::new(&[], start);
        let connection = ConnectionHandle::new();
        let live = [connection.id().clone()];
        let every = Duration::from_secs(30);
        assert!(upkeep.due_pings(start, every, &live).is_empty());
        assert!(
            upkeep
                .due_pings(start + Duration::from_secs(29), every, &live)
                .is_empty()
        );
        let due = upkeep.due_pings(start + every, every, &live);
        assert_eq!(due.len(), 1);
        // No second ping while one is outstanding, however long it takes.
        assert!(upkeep.due_pings(start + every * 5, every, &live).is_empty());
        assert!(!upkeep.ping_finished(connection.id(), [9; 8]));
        assert!(upkeep.ping_finished(connection.id(), due[0].token));
        assert!(!upkeep.ping_finished(connection.id(), due[0].token));
        let next = upkeep.due_pings(start + every * 6, every, &live);
        assert_eq!(next.len(), 1);
        assert_ne!(next[0].token, due[0].token);
    }

    // Connections that disappear are forgotten, so the schedule never grows with churn.
    #[test]
    fn closed_connections_are_forgotten() {
        let start = Instant::now();
        let mut upkeep = Upkeep::new(&[], start);
        let a = ConnectionHandle::new();
        upkeep.due_pings(start, Duration::from_secs(1), &[a.id().clone()]);
        assert_eq!(upkeep.health.len(), 1);
        upkeep.due_pings(start, Duration::from_secs(1), &[]);
        assert!(upkeep.health.is_empty());
    }
}
