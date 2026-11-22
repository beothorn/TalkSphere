//! Bounded per-identity rolling-minute counters.
//!
//! Callers: the node loop counts requests and failures per authenticated identity and compares
//! them with the quota policy's limits. Each identity has sixty one-second buckets, so the
//! count covers exactly the last sixty seconds; the table holds at most [`MAXIMUM_PEERS`]
//! identities and evicts the least recently touched when full, so memory stays bounded even
//! under a flood of new identities.
use crate::modules::identity::api::IdentityId;
use std::collections::BTreeMap;

/// Most identities counted at once.
pub const MAXIMUM_PEERS: usize = 4096;
/// Seconds in the rolling window.
pub const WINDOW_SECONDS: u64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Window {
    buckets: [u32; WINDOW_SECONDS as usize],
    last: u64,
}
impl Window {
    // An empty window positioned at `now`.
    fn new(now: u64) -> Self {
        Self {
            buckets: [0; WINDOW_SECONDS as usize],
            last: now,
        }
    }

    // Zero the buckets for the seconds that passed since the last update.
    fn advance(&mut self, now: u64) {
        if now < self.last {
            return;
        }
        let passed = now - self.last;
        if passed >= WINDOW_SECONDS {
            self.buckets = [0; WINDOW_SECONDS as usize];
        } else {
            for second in self.last + 1..=now {
                self.buckets[(second % WINDOW_SECONDS) as usize] = 0;
            }
        }
        self.last = now;
    }

    // Events inside the last sixty seconds.
    fn total(&self) -> u32 {
        self.buckets.iter().fold(0u32, |t, n| t.saturating_add(*n))
    }
}

/// Rolling-minute counters keyed by identity.
#[derive(Debug, Clone, Default)]
pub struct Rates {
    peers: BTreeMap<IdentityId, Window>,
}

impl Rates {
    /// An empty table.
    pub fn new() -> Self {
        tracing::trace!("Creating an empty rate table");
        Self::default()
    }

    /// Count one event for `peer` at Unix time `now` unless the last minute already holds `limit`
    /// events. Returns whether the event is within the limit; a refused event is not counted.
    pub fn hit(&mut self, peer: IdentityId, now: u64, limit: u32) -> bool {
        tracing::trace!("Counting one rate-limited event");
        if !self.peers.contains_key(&peer) && self.peers.len() >= MAXIMUM_PEERS {
            self.evict();
        }
        let window = self.peers.entry(peer).or_insert_with(|| Window::new(now));
        window.advance(now);
        if window.total() >= limit {
            return false;
        }
        let slot = (now % WINDOW_SECONDS) as usize;
        window.buckets[slot] = window.buckets[slot].saturating_add(1);
        true
    }

    /// Events counted for `peer` in the last minute.
    pub fn count(&self, peer: IdentityId, now: u64) -> u32 {
        tracing::trace!("Reading a rate count");
        self.peers.get(&peer).map_or(0, |w| {
            let mut copy = *w;
            copy.advance(now);
            copy.total()
        })
    }

    /// Identities currently counted.
    pub fn len(&self) -> usize {
        tracing::trace!("Counting rate-limited identities");
        self.peers.len()
    }

    /// Whether no identity is counted.
    pub fn is_empty(&self) -> bool {
        tracing::trace!("Checking for an empty rate table");
        self.peers.is_empty()
    }

    // Drop the least recently touched identity.
    fn evict(&mut self) {
        let victim = self
            .peers
            .iter()
            .min_by_key(|(id, w)| (w.last, **id))
            .map(|(id, _)| *id);
        if let Some(id) = victim {
            self.peers.remove(&id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A distinct identity per byte.
    fn id(n: u8) -> IdentityId {
        IdentityId([n; 32])
    }

    // Events up to the limit pass, the next one is refused and is not counted, and one second later
    // nothing has expired yet because the window is a full minute.
    #[test]
    fn the_limit_applies_within_the_window() {
        let mut rates = Rates::new();
        assert!(rates.hit(id(1), 100, 3));
        assert!(rates.hit(id(1), 100, 3));
        assert!(rates.hit(id(1), 101, 3));
        assert!(!rates.hit(id(1), 102, 3));
        assert_eq!(rates.count(id(1), 102), 3);
        assert!(!rates.hit(id(1), 159, 3));
    }

    // The window slides: events leave it one second at a time, exactly sixty seconds after they happened.
    #[test]
    fn events_leave_the_window_after_sixty_seconds() {
        let mut rates = Rates::new();
        rates.hit(id(1), 100, 2);
        rates.hit(id(1), 130, 2);
        assert!(!rates.hit(id(1), 159, 2));
        // At 160 the event at 100 is 60 seconds old and no longer counted.
        assert!(rates.hit(id(1), 160, 2));
        assert_eq!(rates.count(id(1), 160), 2);
        assert!(!rates.hit(id(1), 161, 2));
        assert_eq!(rates.count(id(1), 400), 0);
    }

    // Identities are counted independently, and a limit of one allows exactly one event per minute.
    #[test]
    fn identities_are_independent() {
        let mut rates = Rates::new();
        assert!(rates.hit(id(1), 5, 1));
        assert!(!rates.hit(id(1), 6, 1));
        assert!(rates.hit(id(2), 6, 1));
        assert!(rates.hit(id(1), 65, 1));
    }

    // A clock that moves backwards never panics or loosens the limit.
    #[test]
    fn a_backwards_clock_is_tolerated() {
        let mut rates = Rates::new();
        assert!(rates.hit(id(1), 200, 1));
        assert!(!rates.hit(id(1), 150, 1));
        assert_eq!(rates.count(id(1), 150), 1);
    }

    // The table never exceeds its bound and evicts the least recently touched identity.
    #[test]
    fn the_table_is_bounded() {
        let mut rates = Rates::new();
        for n in 0..(MAXIMUM_PEERS + 10) {
            let mut bytes = [0u8; 32];
            bytes[..4].copy_from_slice(&(n as u32).to_be_bytes());
            rates.hit(IdentityId(bytes), n as u64, 5);
        }
        assert_eq!(rates.len(), MAXIMUM_PEERS);
        assert_eq!(rates.count(IdentityId([0; 32]), 5000), 0);
    }

    // A very large limit never overflows the counters.
    #[test]
    fn unlimited_limits_do_not_overflow() {
        let mut rates = Rates::new();
        for _ in 0..1000 {
            assert!(rates.hit(id(1), 7, u32::MAX));
        }
        assert_eq!(rates.count(id(1), 7), 1000);
    }
}
