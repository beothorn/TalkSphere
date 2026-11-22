//! Conservative default policy that reproduces the minimum node's behavior.
//!
//! Callers: node composition when no `policy.json` exists, and tests. Every decision here
//! is what the minimum node already did: admit every authenticated peer, keep trust and
//! credits operator-controlled, and publish no wanted offering.
use super::contracts::{
    Admission, Candidate, Connection, Credit, Event, Grant, Limits, PeerView, Policy, Quota,
    Redundancy, Trust, Verdict, Wanted,
};
use crate::modules::identity::api::IdentityId;
use std::sync::Arc;

/// Default pending operations per identity, equal to the node's former `capacity.perOwner`.
pub const DEFAULT_PENDING: usize = 8;
/// Default replica target: CR2SE's minimum for calling data redundant.
pub const DEFAULT_TARGET: usize = 2;
/// Default seconds between checks of one replica.
pub const DEFAULT_CHECK_SECONDS: u64 = 3600;

/// The conservative implementation of every contract.
#[derive(Debug, Clone, Copy, Default)]
pub struct Defaults;

impl Defaults {
    /// Bundle the defaults as one [`Policy`].
    pub fn policy() -> Policy {
        tracing::trace!("Building the conservative default policy");
        let defaults = Arc::new(Self);
        Policy {
            admission: defaults.clone(),
            quota: defaults.clone(),
            trust: defaults.clone(),
            credit: defaults.clone(),
            redundancy: defaults.clone(),
            connection: defaults,
        }
    }
}

impl Admission for Defaults {
    // Every authenticated peer stays connected; paid work is still gated by Ledger trust.
    fn admit(&self, _peer: &PeerView, _live: usize, _from_peer: usize) -> Verdict {
        tracing::trace!("Admitting an authenticated peer by default");
        Verdict::Allow
    }
}

impl Quota for Defaults {
    // Only the former simultaneous-operation bound; no rate limits existed per identity.
    fn limits(&self, _peer: &PeerView) -> Limits {
        tracing::trace!("Reading the default per-identity limits");
        Limits {
            pending: DEFAULT_PENDING,
            requests_per_minute: u32::MAX,
            failures_per_minute: u32::MAX,
        }
    }
}

impl Trust for Defaults {
    // Trust changes only when the operator sets it.
    fn adjust(&self, _peer: &PeerView, _event: Event) -> Option<u32> {
        tracing::trace!("Leaving trust unchanged by default");
        None
    }
}

impl Credit for Defaults {
    // No automatic grants.
    fn bootstrap(&self, _peer: &PeerView, _issued_total: u64) -> Option<Grant> {
        tracing::trace!("Issuing no bootstrap credits by default");
        None
    }
    // No wanted offering.
    fn wanted(&self) -> Option<Wanted> {
        tracing::trace!("Publishing no wanted offering by default");
        None
    }
    // The least the sender will accept; never more than the minimum without a policy.
    fn release(&self, _cumulative_payments: u64, minimum: u64) -> u64 {
        tracing::trace!("Offering the minimum release credits by default");
        minimum
    }
}

impl Redundancy for Defaults {
    fn target(&self) -> usize {
        tracing::trace!("Reading the default replica target");
        DEFAULT_TARGET
    }
    // Connected, nonexcluded peers in the given order; callers present candidates best first.
    fn select(
        &self,
        candidates: &[Candidate],
        have: &[IdentityId],
        need: usize,
    ) -> Vec<IdentityId> {
        tracing::trace!("Selecting replica holders in presented order");
        candidates
            .iter()
            .filter(|c| c.connected && !have.contains(&c.peer))
            .map(|c| c.peer)
            .take(need)
            .collect()
    }
    // One interval after the last check, staggered by one tenth of an interval per replica index.
    fn next_check(&self, last_checked: u64, index: usize) -> u64 {
        tracing::trace!("Scheduling the next replica check");
        let stagger = DEFAULT_CHECK_SECONDS / 10 * (index as u64 % 10);
        last_checked
            .saturating_add(DEFAULT_CHECK_SECONDS)
            .saturating_add(stagger)
    }
}

impl Connection for Defaults {
    fn ping_interval_seconds(&self) -> u64 {
        tracing::trace!("Reading the default ping interval");
        30
    }
    fn ping_timeout_seconds(&self) -> u64 {
        tracing::trace!("Reading the default ping timeout");
        10
    }
    // Doubling delay capped at one minute.
    fn reconnect_delay_seconds(&self, attempt: u32) -> u64 {
        tracing::trace!("Computing the default reconnect delay");
        1u64.checked_shl(attempt.min(6)).unwrap_or(64).min(60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composition::policy::contracts::Summary;

    // A neutral view for decisions that ignore history.
    fn view() -> PeerView {
        PeerView {
            peer: IdentityId([1; 32]),
            trust_millionths: 0,
            summary: Summary::default(),
            now_seconds: 1000,
        }
    }

    // The defaults reproduce the minimum node: admit everyone, no rates, no trust or credit changes.
    #[test]
    fn defaults_match_the_minimum_node() {
        let policy = Defaults::policy();
        assert_eq!(policy.admission.admit(&view(), 63, 5), Verdict::Allow);
        let limits = policy.quota.limits(&view());
        assert_eq!(limits.pending, DEFAULT_PENDING);
        assert_eq!(limits.requests_per_minute, u32::MAX);
        assert_eq!(policy.trust.adjust(&view(), Event::CheckFailed), None);
        assert_eq!(policy.credit.bootstrap(&view(), 0), None);
        assert_eq!(policy.credit.wanted(), None);
        assert_eq!(policy.credit.release(100, 4), 4);
    }

    // Selection skips disconnected candidates and existing holders and never exceeds `need`.
    #[test]
    fn selection_requires_connected_new_peers() {
        let candidate = |n: u8, connected| Candidate {
            peer: IdentityId([n; 32]),
            connected,
            summary: Summary::default(),
            trust_millionths: 0,
        };
        let all = [
            candidate(1, true),
            candidate(2, false),
            candidate(3, true),
            candidate(4, true),
        ];
        let have = [IdentityId([1; 32])];
        let picked = Defaults.select(&all, &have, 2);
        assert_eq!(picked, vec![IdentityId([3; 32]), IdentityId([4; 32])]);
        assert!(Defaults.select(&all, &have, 0).is_empty());
    }

    // Check times move forward, differ per replica index and never overflow.
    #[test]
    fn checks_are_staggered_and_saturating() {
        let first = Defaults.next_check(1000, 0);
        assert_eq!(first, 1000 + DEFAULT_CHECK_SECONDS);
        assert_ne!(Defaults.next_check(1000, 1), first);
        assert_eq!(Defaults.next_check(u64::MAX, 3), u64::MAX);
    }

    // Reconnect delay doubles, then stays at the cap for every larger attempt number.
    #[test]
    fn reconnect_delay_doubles_to_the_cap() {
        let delays: Vec<u64> = (0..9)
            .map(|a| Defaults.reconnect_delay_seconds(a))
            .collect();
        assert_eq!(delays, vec![1, 2, 4, 8, 16, 32, 60, 60, 60]);
        assert_eq!(Defaults.reconnect_delay_seconds(u32::MAX), 60);
    }
}
