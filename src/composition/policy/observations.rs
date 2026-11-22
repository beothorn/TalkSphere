//! Bounded per-peer observations that policy decisions read.
//!
//! Callers: node composition records facts as they happen and builds a [`PeerView`] summary
//! for each decision. Everything is bounded: a fixed number of peers, saturating counters,
//! and age-out of idle entries. Entries are keyed by authenticated identity only; no
//! peer-supplied text is stored, so nothing here is a trusted fact about the peer beyond
//! what this node itself observed.
use super::contracts::Summary;
use crate::modules::identity::{api::IdentityId, text};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Most peers remembered.
pub const MAXIMUM_PEERS: usize = 4096;
/// Idle entries without history older than this are dropped, in seconds.
pub const IDLE_SECONDS: u64 = 30 * 86_400;
/// Idle entries that earned something are kept this long, in seconds.
pub const HISTORY_SECONDS: u64 = 365 * 86_400;

/// What happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// An authenticated connection was accepted.
    Connected,
    /// A contribution was verified.
    ContributionVerified,
    /// A paid check passed.
    CheckPassed,
    /// A paid check failed with matching negative evidence.
    CheckFailed,
    /// A request was refused by quota, rate or capacity.
    Refused,
    /// An operation attributable to the peer failed.
    Failed,
}

/// Why a stored document was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invalid {
    /// Not the expected bounded JSON shape.
    Malformed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Entry {
    summary: Summary,
    last_seen: u64,
}
impl Entry {
    // Whether the peer ever contributed or was granted anything; such entries outlive anonymous ones.
    fn has_history(&self) -> bool {
        let s = &self.summary;
        s.contributions > 0 || s.checks_passed > 0 || s.bootstrap_credits > 0
    }
}

/// The in-memory observation table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Observations {
    peers: BTreeMap<IdentityId, Entry>,
}

impl Observations {
    /// An empty table.
    pub fn new() -> Self {
        tracing::trace!("Creating an empty observation table");
        Self::default()
    }

    /// Number of remembered peers.
    pub fn len(&self) -> usize {
        tracing::trace!("Counting remembered peers");
        self.peers.len()
    }

    /// Whether no peer is remembered.
    pub fn is_empty(&self) -> bool {
        tracing::trace!("Checking for an empty observation table");
        self.peers.is_empty()
    }

    /// Record one observation at Unix time `now`; counters saturate and a full table evicts one entry.
    pub fn record(&mut self, peer: IdentityId, kind: Kind, now: u64) {
        tracing::trace!("Recording a peer observation");
        let entry = self.entry(peer, now);
        let s = &mut entry.summary;
        let counter = match kind {
            Kind::Connected => &mut s.connections,
            Kind::ContributionVerified => &mut s.contributions,
            Kind::CheckPassed => &mut s.checks_passed,
            Kind::CheckFailed => &mut s.checks_failed,
            Kind::Refused => &mut s.refusals,
            Kind::Failed => &mut s.failures,
        };
        *counter = counter.saturating_add(1);
    }

    /// Record credits issued to `peer` by bootstrap policy.
    pub fn record_bootstrap(&mut self, peer: IdentityId, credits: u64, now: u64) {
        tracing::trace!("Recording bootstrap credits");
        let s = &mut self.entry(peer, now).summary;
        s.bootstrap_credits = s.bootstrap_credits.saturating_add(credits);
    }

    /// What is known about `peer`; unknown peers have an empty summary.
    pub fn summary(&self, peer: IdentityId) -> Summary {
        tracing::trace!("Reading a peer summary");
        self.peers.get(&peer).map(|e| e.summary).unwrap_or_default()
    }

    /// Total bootstrap credits recorded across all peers.
    pub fn bootstrap_total(&self) -> u64 {
        tracing::trace!("Summing bootstrap credits");
        self.peers
            .values()
            .fold(0u64, |t, e| t.saturating_add(e.summary.bootstrap_credits))
    }

    /// Drop entries idle for too long: anonymous ones after [`IDLE_SECONDS`], others after
    /// [`HISTORY_SECONDS`]. Entries holding bootstrap credits are never dropped, because
    /// forgetting a grant would allow a second one. Returns the number dropped.
    pub fn age_out(&mut self, now: u64) -> usize {
        tracing::trace!("Ageing out idle peer observations");
        let before = self.peers.len();
        self.peers.retain(|_, e| {
            let idle = now.saturating_sub(e.last_seen);
            let limit = if e.has_history() {
                HISTORY_SECONDS
            } else {
                IDLE_SECONDS
            };
            e.summary.bootstrap_credits > 0 || idle <= limit
        });
        before - self.peers.len()
    }

    // The entry for `peer`, created at `now`; a full table first evicts the least valuable entry.
    fn entry(&mut self, peer: IdentityId, now: u64) -> &mut Entry {
        if !self.peers.contains_key(&peer) && self.peers.len() >= MAXIMUM_PEERS {
            self.evict();
        }
        let entry = self.peers.entry(peer).or_insert(Entry {
            summary: Summary {
                first_seen: Some(now),
                ..Summary::default()
            },
            last_seen: now,
        });
        entry.last_seen = entry.last_seen.max(now);
        entry
    }

    // Evict anonymous peers before ones with history, oldest first, never one holding bootstrap credits
    // unless nothing else remains (a flood of new identities then cannot erase granted peers first).
    fn evict(&mut self) {
        let victim = self
            .peers
            .iter()
            .min_by_key(|(id, e)| {
                (
                    e.summary.bootstrap_credits > 0,
                    e.has_history(),
                    e.last_seen,
                    **id,
                )
            })
            .map(|(id, _)| *id);
        if let Some(id) = victim {
            tracing::debug!("Evicting the least valuable peer observation");
            self.peers.remove(&id);
        }
    }

    /// Canonical document for durable storage.
    pub fn to_json(&self) -> Value {
        tracing::trace!("Serializing peer observations");
        let peers: Vec<Value> = self
            .peers
            .iter()
            .map(|(id, e)| {
                let s = &e.summary;
                json!({
                    "id": text::format(*id), "firstSeen": s.first_seen, "lastSeen": e.last_seen,
                    "connections": s.connections, "contributions": s.contributions,
                    "checksPassed": s.checks_passed, "checksFailed": s.checks_failed,
                    "refusals": s.refusals, "failures": s.failures,
                    "bootstrapCredits": s.bootstrap_credits,
                })
            })
            .collect();
        json!({"version": 1, "peers": peers})
    }

    /// Rebuild a table from a stored document.
    /// # Errors
    /// Any wrong shape, unknown member, bad identity, duplicate or more than [`MAXIMUM_PEERS`] peers.
    pub fn from_json(document: &Value) -> Result<Self, Invalid> {
        tracing::trace!("Parsing stored peer observations");
        let root = document.as_object().ok_or(Invalid::Malformed)?;
        if root.len() != 2 || root.get("version").and_then(Value::as_u64) != Some(1) {
            return Err(Invalid::Malformed);
        }
        let list = root
            .get("peers")
            .and_then(Value::as_array)
            .ok_or(Invalid::Malformed)?;
        if list.len() > MAXIMUM_PEERS {
            return Err(Invalid::Malformed);
        }
        let mut peers = BTreeMap::new();
        for item in list {
            let (id, entry) = parse_entry(item)?;
            if peers.insert(id, entry).is_some() {
                return Err(Invalid::Malformed);
            }
        }
        Ok(Self { peers })
    }
}

// One stored peer; every member is required and exact.
fn parse_entry(item: &Value) -> Result<(IdentityId, Entry), Invalid> {
    let o = item.as_object().ok_or(Invalid::Malformed)?;
    if o.len() != 10 {
        return Err(Invalid::Malformed);
    }
    let number = |name: &str| {
        o.get(name)
            .and_then(Value::as_u64)
            .ok_or(Invalid::Malformed)
    };
    let count =
        |name: &str| number(name).and_then(|n| u32::try_from(n).map_err(|_| Invalid::Malformed));
    let id = o
        .get("id")
        .and_then(Value::as_str)
        .and_then(|t| text::parse(t).ok())
        .ok_or(Invalid::Malformed)?;
    let first_seen = match o.get("firstSeen") {
        Some(Value::Null) => None,
        Some(v) => Some(v.as_u64().ok_or(Invalid::Malformed)?),
        None => return Err(Invalid::Malformed),
    };
    Ok((
        id,
        Entry {
            last_seen: number("lastSeen")?,
            summary: Summary {
                first_seen,
                connections: count("connections")?,
                contributions: count("contributions")?,
                checks_passed: count("checksPassed")?,
                checks_failed: count("checksFailed")?,
                refusals: count("refusals")?,
                failures: count("failures")?,
                bootstrap_credits: number("bootstrapCredits")?,
            },
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // A distinct identity per byte.
    fn id(n: u8) -> IdentityId {
        IdentityId([n; 32])
    }

    // Recording counts each kind separately, remembers first sight and treats unknown peers as empty.
    #[test]
    fn recording_counts_by_kind() {
        let mut o = Observations::new();
        assert_eq!(o.summary(id(1)), Summary::default());
        o.record(id(1), Kind::Connected, 100);
        o.record(id(1), Kind::Connected, 150);
        o.record(id(1), Kind::CheckFailed, 160);
        o.record(id(1), Kind::Refused, 170);
        let s = o.summary(id(1));
        assert_eq!(
            (s.first_seen, s.connections, s.checks_failed, s.refusals),
            (Some(100), 2, 1, 1)
        );
        assert_eq!(o.summary(id(2)), Summary::default());
    }

    // Counters saturate instead of wrapping.
    #[test]
    fn counters_saturate() {
        let mut o = Observations::new();
        o.record(id(1), Kind::Failed, 1);
        o.peers.get_mut(&id(1)).unwrap().summary.failures = u32::MAX;
        o.record(id(1), Kind::Failed, 2);
        assert_eq!(o.summary(id(1)).failures, u32::MAX);
        o.record_bootstrap(id(1), u64::MAX, 3);
        o.record_bootstrap(id(1), 5, 4);
        assert_eq!(o.summary(id(1)).bootstrap_credits, u64::MAX);
    }

    // A full table never grows: anonymous oldest peers go first and granted peers survive a flood.
    #[test]
    fn the_table_is_bounded_and_keeps_granted_peers() {
        let mut o = Observations::new();
        o.record_bootstrap(id(255), 10, 1);
        for n in 0..(MAXIMUM_PEERS + 100) {
            let mut bytes = [0u8; 32];
            bytes[..4].copy_from_slice(&(n as u32).to_be_bytes());
            o.record(IdentityId(bytes), Kind::Connected, 10 + n as u64);
        }
        assert_eq!(o.len(), MAXIMUM_PEERS);
        assert_eq!(o.summary(id(255)).bootstrap_credits, 10);
    }

    // Ageing drops idle anonymous peers, keeps earners longer and never forgets a bootstrap grant.
    #[test]
    fn ageing_keeps_what_must_be_remembered() {
        let mut o = Observations::new();
        o.record(id(1), Kind::Connected, 0);
        o.record(id(2), Kind::ContributionVerified, 0);
        o.record_bootstrap(id(3), 7, 0);
        assert_eq!(o.age_out(IDLE_SECONDS), 0);
        assert_eq!(o.age_out(IDLE_SECONDS + 1), 1);
        assert_eq!(o.age_out(HISTORY_SECONDS + 1), 1);
        assert_eq!(o.len(), 1);
        assert_eq!(o.bootstrap_total(), 7);
    }

    // The canonical document round trips exactly.
    #[test]
    fn serialization_round_trips() {
        let mut o = Observations::new();
        o.record(id(1), Kind::Connected, 5);
        o.record(id(2), Kind::CheckPassed, 6);
        o.record_bootstrap(id(2), 9, 7);
        assert_eq!(Observations::from_json(&o.to_json()).unwrap(), o);
        assert_eq!(
            Observations::from_json(&Observations::new().to_json()).unwrap(),
            Observations::new()
        );
    }

    // Wrong shapes, unknown members, bad identities, duplicates and too many peers are refused.
    #[test]
    fn corrupt_documents_are_refused() {
        let mut o = Observations::new();
        o.record(id(1), Kind::Connected, 5);
        let good = o.to_json();
        for mutate in [
            |v: &mut Value| v["version"] = json!(2),
            |v: &mut Value| v["extra"] = json!(1),
            |v: &mut Value| v["peers"][0]["id"] = json!("nonsense"),
            |v: &mut Value| v["peers"][0]["failures"] = json!(-1),
            |v: &mut Value| v["peers"][0]["failures"] = json!(4294967296u64),
            |v: &mut Value| v["peers"][0]["unknown"] = json!(1),
            |v: &mut Value| {
                let first = v["peers"][0].clone();
                v["peers"].as_array_mut().unwrap().push(first);
            },
        ] {
            let mut bad = good.clone();
            mutate(&mut bad);
            assert_eq!(Observations::from_json(&bad), Err(Invalid::Malformed));
        }
        assert_eq!(Observations::from_json(&json!([])), Err(Invalid::Malformed));
        let many: Vec<Value> = (0..=MAXIMUM_PEERS)
            .map(|_| good["peers"][0].clone())
            .collect();
        assert_eq!(
            Observations::from_json(&json!({"version":1,"peers":many})),
            Err(Invalid::Malformed)
        );
    }
}
