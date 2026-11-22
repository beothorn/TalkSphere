//! Policy implementations driven by validated [`Params`].
//!
//! Callers: node composition. [`Configured`] implements all six contracts from one parameter
//! set; `Configured::new(Params::default())` reproduces the minimum node. Decisions are pure
//! functions of the peer view and the parameters.
use super::{
    contracts::{
        Admission, Candidate, Connection, Credit, Event, Grant, Limits, PeerView, Policy, Quota,
        Reason, Redundancy, Trust, Verdict, Wanted,
    },
    params::Params,
};
use crate::modules::identity::api::IdentityId;
use std::sync::Arc;

/// One parameter set serving every contract.
#[derive(Debug, Clone, Copy)]
pub struct Configured {
    params: Params,
}

impl Configured {
    /// Wrap validated parameters.
    pub fn new(params: Params) -> Self {
        tracing::trace!("Creating the configured policy");
        Self { params }
    }

    /// The parameters this policy decides from.
    pub fn params(&self) -> &Params {
        tracing::trace!("Reading the configured policy parameters");
        &self.params
    }

    /// Bundle one shared instance as every contract.
    pub fn policy(params: Params) -> Policy {
        tracing::trace!("Bundling the configured policy");
        let shared = Arc::new(Self::new(params));
        Policy {
            admission: shared.clone(),
            quota: shared.clone(),
            trust: shared.clone(),
            credit: shared.clone(),
            redundancy: shared.clone(),
            connection: shared,
        }
    }
}

impl Admission for Configured {
    // Refuse when the node or the peer is at its connection limit.
    fn admit(&self, _peer: &PeerView, live: usize, from_peer: usize) -> Verdict {
        tracing::trace!("Deciding connection admission");
        let a = &self.params.admission;
        if live >= a.connections {
            Verdict::Refuse(Reason::ConnectionLimit)
        } else if from_peer >= a.per_peer_connections {
            Verdict::Refuse(Reason::PeerConnectionLimit)
        } else {
            Verdict::Allow
        }
    }
}

impl Quota for Configured {
    fn limits(&self, _peer: &PeerView) -> Limits {
        tracing::trace!("Reading per-identity limits");
        let q = &self.params.quota;
        Limits {
            pending: q.pending,
            requests_per_minute: q.requests_per_minute,
            failures_per_minute: q.failures_per_minute,
        }
    }
}

impl Trust for Configured {
    // Steps are bounded: raises stop at the ceiling and never lower trust the operator set above it.
    fn adjust(&self, peer: &PeerView, event: Event) -> Option<u32> {
        tracing::trace!("Deciding a trust adjustment");
        let t = &self.params.trust;
        let current = peer.trust_millionths;
        let new = match event {
            Event::CheckPassed => raise(current, t.check_passed, t.ceiling),
            Event::ContributionVerified => raise(current, t.contribution, t.ceiling),
            Event::CheckFailed => current.saturating_sub(t.check_failed),
            Event::Failed => current.saturating_sub(t.failure),
        };
        (new != current).then_some(new)
    }
}

// Raise by `step`, never above `ceiling` and never lowering an already higher value.
fn raise(current: u32, step: u32, ceiling: u32) -> u32 {
    if current >= ceiling {
        return current;
    }
    current.saturating_add(step).min(ceiling)
}

impl Credit for Configured {
    // One grant per identity, within the total bootstrap budget.
    fn bootstrap(&self, peer: &PeerView, issued_total: u64) -> Option<Grant> {
        tracing::trace!("Deciding a bootstrap grant");
        let c = &self.params.credit;
        if c.bootstrap_credits == 0 || peer.summary.bootstrap_credits > 0 {
            return None;
        }
        let after = issued_total.checked_add(c.bootstrap_credits)?;
        (after <= c.bootstrap_budget).then_some(Grant {
            credits: c.bootstrap_credits,
            trust_millionths: c.bootstrap_trust,
        })
    }
    fn wanted(&self) -> Option<Wanted> {
        tracing::trace!("Reading the wanted offering terms");
        self.params.credit.wanted
    }
    // The configured share of cumulative payments, never below the sender's minimum or above the payments.
    fn release(&self, cumulative_payments: u64, minimum: u64) -> u64 {
        tracing::trace!("Computing early-release credits");
        let share =
            u128::from(cumulative_payments) * u128::from(self.params.credit.release_percent) / 100;
        let share = u64::try_from(share).unwrap_or(u64::MAX);
        share.max(minimum).min(cumulative_payments.max(minimum))
    }
}

impl Redundancy for Configured {
    fn target(&self) -> usize {
        tracing::trace!("Reading the replica target");
        self.params.redundancy.target
    }
    // Connected new peers with the fewest recorded failures first, then highest trust, then input order.
    fn select(
        &self,
        candidates: &[Candidate],
        have: &[IdentityId],
        need: usize,
    ) -> Vec<IdentityId> {
        tracing::trace!("Selecting replica holders");
        let mut eligible: Vec<&Candidate> = candidates
            .iter()
            .filter(|c| c.connected && !have.contains(&c.peer))
            .collect();
        eligible.sort_by_key(|c| {
            (
                c.summary.failures + c.summary.checks_failed,
                std::cmp::Reverse(c.trust_millionths),
            )
        });
        eligible.into_iter().map(|c| c.peer).take(need).collect()
    }
    // One interval after the last check plus a per-index offset, so replicas never fall due together.
    fn next_check(&self, last_checked: u64, index: usize) -> u64 {
        tracing::trace!("Scheduling the next replica check");
        let r = &self.params.redundancy;
        let stagger =
            r.check_seconds * u64::from(r.stagger_percent) / 100 * (index as u64 % 10) / 10;
        last_checked
            .saturating_add(r.check_seconds)
            .saturating_add(stagger)
    }
    fn renew_before_seconds(&self) -> u64 {
        tracing::trace!("Reading the renewal lead time");
        self.params.redundancy.renew_before_seconds
    }
    fn max_spend(&self) -> u64 {
        tracing::trace!("Reading the maintenance spending bound");
        self.params.redundancy.max_spend
    }
    fn retry_seconds(&self) -> u64 {
        tracing::trace!("Reading the maintenance retry delay");
        self.params.redundancy.retry_seconds
    }
}

impl Connection for Configured {
    fn ping_interval_seconds(&self) -> u64 {
        tracing::trace!("Reading the ping interval");
        self.params.connection.ping_seconds
    }
    fn ping_timeout_seconds(&self) -> u64 {
        tracing::trace!("Reading the ping timeout");
        self.params.connection.ping_timeout_seconds
    }
    // Doubling delay capped at the configured maximum.
    fn reconnect_delay_seconds(&self, attempt: u32) -> u64 {
        tracing::trace!("Computing the reconnect delay");
        let cap = self.params.connection.reconnect_max_seconds;
        1u64.checked_shl(attempt.min(32)).unwrap_or(cap).min(cap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composition::policy::contracts::Summary;

    // A view with the given trust and bootstrap history.
    fn view(trust: u32, bootstrapped: u64) -> PeerView {
        PeerView {
            peer: IdentityId([1; 32]),
            trust_millionths: trust,
            summary: Summary {
                bootstrap_credits: bootstrapped,
                ..Summary::default()
            },
            now_seconds: 1000,
        }
    }

    // A policy parsed from a document, so tests exercise the same path operators use.
    fn policy(doc: &str) -> Configured {
        Configured::new(Params::parse(doc.as_bytes()).unwrap())
    }

    // The default parameters reproduce the minimum node: admit, no rates, no trust or credit changes.
    #[test]
    fn defaults_match_the_minimum_node() {
        let p = Configured::new(Params::default());
        assert_eq!(p.admit(&view(0, 0), 63, 63), Verdict::Allow);
        assert_eq!(p.limits(&view(0, 0)).pending, 8);
        assert_eq!(p.limits(&view(0, 0)).requests_per_minute, u32::MAX);
        assert_eq!(p.adjust(&view(5, 0), Event::CheckFailed), None);
        assert_eq!(p.adjust(&view(5, 0), Event::CheckPassed), None);
        assert_eq!(p.bootstrap(&view(0, 0), 0), None);
        assert_eq!(p.wanted(), None);
        assert_eq!(p.release(100, 4), 4);
        assert_eq!(p.target(), 2);
    }

    // Admission refuses at the node limit before the per-peer limit, and the boundary itself is allowed.
    #[test]
    fn admission_respects_both_limits() {
        let p = policy(r#"{"version":1,"admission":{"connections":4,"perPeerConnections":2}}"#);
        assert_eq!(p.admit(&view(0, 0), 3, 1), Verdict::Allow);
        assert_eq!(
            p.admit(&view(0, 0), 4, 0),
            Verdict::Refuse(Reason::ConnectionLimit)
        );
        assert_eq!(
            p.admit(&view(0, 0), 3, 2),
            Verdict::Refuse(Reason::PeerConnectionLimit)
        );
    }

    // Trust raises stop at the ceiling, never lower a higher operator value and falls floor at zero.
    #[test]
    fn trust_steps_are_bounded() {
        let p = policy(
            r#"{"version":1,"trust":{"checkPassed":300,"checkFailed":500,"contribution":100,"failure":50,"ceiling":1000}}"#,
        );
        assert_eq!(p.adjust(&view(0, 0), Event::CheckPassed), Some(300));
        assert_eq!(p.adjust(&view(900, 0), Event::CheckPassed), Some(1000));
        assert_eq!(p.adjust(&view(1000, 0), Event::CheckPassed), None);
        assert_eq!(p.adjust(&view(5000, 0), Event::ContributionVerified), None);
        assert_eq!(p.adjust(&view(400, 0), Event::CheckFailed), Some(0));
        assert_eq!(p.adjust(&view(0, 0), Event::CheckFailed), None);
        assert_eq!(p.adjust(&view(100, 0), Event::Failed), Some(50));
    }

    // Bootstrap grants once per identity and stops at the total budget, including near overflow.
    #[test]
    fn bootstrap_is_finite() {
        let p = policy(
            r#"{"version":1,"credit":{"bootstrapCredits":10,"bootstrapTrust":1,"bootstrapBudget":25}}"#,
        );
        assert_eq!(
            p.bootstrap(&view(0, 0), 0),
            Some(Grant {
                credits: 10,
                trust_millionths: 1
            })
        );
        assert_eq!(p.bootstrap(&view(0, 10), 10), None);
        assert!(p.bootstrap(&view(0, 0), 15).is_some());
        assert_eq!(p.bootstrap(&view(0, 0), 16), None);
        assert_eq!(p.bootstrap(&view(0, 0), u64::MAX), None);
    }

    // Release credits follow the percentage but stay between the minimum and the cumulative payments.
    #[test]
    fn release_is_bounded() {
        let p = policy(r#"{"version":1,"credit":{"releasePercent":50}}"#);
        assert_eq!(p.release(100, 4), 50);
        assert_eq!(p.release(100, 70), 70);
        assert_eq!(p.release(100, 150), 150);
        assert_eq!(p.release(u64::MAX, 0), u64::MAX / 2);
    }

    // Selection prefers peers with fewer failures, then higher trust, and skips holders and disconnected peers.
    #[test]
    fn selection_prefers_reliable_peers() {
        let c = |n: u8, connected, failures, trust| Candidate {
            peer: IdentityId([n; 32]),
            connected,
            summary: Summary {
                failures,
                ..Summary::default()
            },
            trust_millionths: trust,
        };
        let all = [
            c(1, true, 2, 9),
            c(2, true, 0, 1),
            c(3, true, 0, 5),
            c(4, false, 0, 9),
            c(5, true, 0, 9),
        ];
        let p = Configured::new(Params::default());
        assert_eq!(
            p.select(&all, &[IdentityId([5; 32])], 3),
            vec![
                IdentityId([3; 32]),
                IdentityId([2; 32]),
                IdentityId([1; 32])
            ]
        );
        assert!(p.select(&all, &[], 0).is_empty());
    }

    // Replica checks are spaced by the interval, differ by index and saturate at the end of time.
    #[test]
    fn checks_are_staggered_and_saturating() {
        let p = policy(r#"{"version":1,"redundancy":{"checkSeconds":1000,"staggerPercent":50}}"#);
        assert_eq!(p.next_check(100, 0), 1100);
        assert!(p.next_check(100, 1) > p.next_check(100, 0));
        assert_ne!(p.next_check(100, 2), p.next_check(100, 3));
        assert_eq!(p.next_check(u64::MAX, 5), u64::MAX);
    }

    // Reconnect delay doubles to the configured cap, including absurd attempt counts.
    #[test]
    fn reconnect_delay_doubles_to_the_cap() {
        let p = policy(r#"{"version":1,"connection":{"reconnectMaxSeconds":20}}"#);
        let delays: Vec<u64> = (0..7).map(|a| p.reconnect_delay_seconds(a)).collect();
        assert_eq!(delays, vec![1, 2, 4, 8, 16, 20, 20]);
        assert_eq!(p.reconnect_delay_seconds(u32::MAX), 20);
    }

    // The bundle shares one policy, so every contract sees the same parameters.
    #[test]
    fn the_bundle_serves_every_contract() {
        let bundle = Configured::policy(Params::default());
        assert_eq!(bundle.redundancy.target(), 2);
        assert_eq!(bundle.connection.ping_interval_seconds(), 30);
        assert_eq!(bundle.connection.ping_timeout_seconds(), 10);
    }
}
