//! The CR2SE redundancy states of a hosted item, computed from observations only.
//!
//! Callers: `node-ctl replicas`. CR2SE Redundancy requires an application that calls data redundant to
//! show the local target and the number of currently confirmed, unexpired agreements separately, to
//! never present an attempt or a requested target as completed redundancy, and to say which
//! observations are stale. This report follows that: the state is derived from confirmed agreements
//! only, it describes observations rather than a guarantee, and staleness and expiry are listed.
use super::book::Item;
use crate::composition::policy::Redundancy;

/// The CR2SE redundancy state of one item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Fewer than two confirmed, unexpired agreements.
    NotRedundant,
    /// At least two but fewer than the local target.
    UnderReplicated,
    /// At least the local target (and at least two).
    TargetSatisfied,
}

impl State {
    /// Stable name for output.
    pub fn name(self) -> &'static str {
        match self {
            Self::NotRedundant => "not redundant",
            Self::UnderReplicated => "under-replicated",
            Self::TargetSatisfied => "target satisfied",
        }
    }
}

/// What can be said about one item at Unix time `now`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The local target: what is wanted, never what exists.
    pub target: usize,
    /// Confirmed agreements that have not expired.
    pub confirmed: usize,
    /// The CR2SE state, from the confirmed count only.
    pub state: State,
    /// Whether the confirmed count reaches the local target (a target of one is met by a single copy
    /// that is still not redundant).
    pub target_met: bool,
    /// Whether any observation calls for repair: a renewal is due, a check is overdue or an agreement expired.
    pub at_risk: bool,
    /// Unexpired replicas whose retention ends within the renewal lead time.
    pub expiring_soon: usize,
    /// Unexpired replicas whose last conclusive check is older than two check intervals (or never happened
    /// and the confirmation is that old).
    pub stale_observations: usize,
    /// Replicas whose retention already ended and that are about to be dropped.
    pub expired: usize,
}

/// Report one item. `check_seconds` is the policy's check interval.
pub fn report(item: &Item, now: u64, policy: &dyn Redundancy, check_seconds: u64) -> Report {
    tracing::trace!("Computing the redundancy report of an item");
    let live = || item.replicas.iter().filter(|r| r.expires_at > now);
    let confirmed = live().count();
    let expired = item.replicas.len() - confirmed;
    let expiring_soon = live()
        .filter(|r| r.expires_at - now <= policy.renew_before_seconds())
        .count();
    let stale_observations = live()
        .filter(|r| {
            let observed = r.last_checked.unwrap_or(r.confirmed_at);
            now.saturating_sub(observed) > check_seconds.saturating_mul(2)
        })
        .count();
    let target = policy.target();
    let state = if confirmed < 2 {
        State::NotRedundant
    } else if confirmed < target {
        State::UnderReplicated
    } else {
        State::TargetSatisfied
    };
    Report {
        target,
        confirmed,
        state,
        target_met: confirmed >= target,
        at_risk: expiring_soon > 0 || stale_observations > 0 || expired > 0,
        expiring_soon,
        stale_observations,
        expired,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composition::node::replicas::book::Replica;
    use crate::composition::policy::{Configured, Params};
    use crate::modules::identity::api::IdentityId;

    // Policy with the given target, a 100-second check interval and renewal within 50 seconds of expiry.
    fn policy(target: usize) -> Configured {
        Configured::new(
            Params::parse(
                format!(
                    r#"{{"version":1,"redundancy":{{"target":{target},"checkSeconds":100,"renewBeforeSeconds":50}}}}"#
                )
                .as_bytes(),
            )
            .unwrap(),
        )
    }

    // An item whose replicas are (provider, expires_at, last_checked), all confirmed at time 0.
    fn item(replicas: &[(u8, u64, Option<u64>)]) -> Item {
        Item {
            id: [1; 16],
            size: 3,
            sealed_size: 44,
            plaintext_hash: [0; 32],
            created_at: 0,
            replicas: replicas
                .iter()
                .map(|(p, expires_at, last_checked)| Replica {
                    provider: IdentityId([*p; 32]),
                    lease: [*p; 32],
                    expires_at: *expires_at,
                    confirmed_at: 0,
                    last_checked: *last_checked,
                })
                .collect(),
        }
    }

    // The state follows the confirmed count: none or one is not redundant, two below the target is
    // under-replicated, the target or more is satisfied.
    #[test]
    fn the_state_follows_the_confirmed_count() {
        let healthy = |n: u8| item(&(1..=n).map(|p| (p, 10_000, Some(90))).collect::<Vec<_>>());
        let state = |n: u8, target| report(&healthy(n), 100, &policy(target), 100).state;
        assert_eq!(state(0, 3), State::NotRedundant);
        assert_eq!(state(1, 3), State::NotRedundant);
        assert_eq!(state(2, 3), State::UnderReplicated);
        assert_eq!(state(3, 3), State::TargetSatisfied);
        assert_eq!(state(4, 3), State::TargetSatisfied);
    }

    // A target of one is met by a single copy, which CR2SE still calls not redundant; the two facts are
    // reported separately so a requested target is never presented as completed redundancy.
    #[test]
    fn a_single_copy_meets_a_target_of_one_without_being_redundant() {
        let one = item(&[(1, 10_000, Some(90))]);
        let r = report(&one, 100, &policy(1), 100);
        assert_eq!(
            (r.state, r.target_met, r.confirmed, r.target),
            (State::NotRedundant, true, 1, 1)
        );
        let none = report(&item(&[]), 100, &policy(1), 100);
        assert_eq!((none.state, none.target_met), (State::NotRedundant, false));
    }

    // Expired agreements are not confirmed, are counted separately and put the item at risk.
    #[test]
    fn expired_agreements_do_not_count() {
        let mixed = item(&[
            (1, 50, Some(40)),
            (2, 10_000, Some(90)),
            (3, 10_000, Some(90)),
        ]);
        let r = report(&mixed, 100, &policy(3), 100);
        assert_eq!(
            (r.confirmed, r.expired, r.state),
            (2, 1, State::UnderReplicated)
        );
        assert!(r.at_risk);
    }

    // Risk comes from a renewal lead time, a stale or never-made check, and nothing else.
    #[test]
    fn risk_comes_from_expiry_and_staleness() {
        let healthy = item(&[(1, 10_000, Some(90)), (2, 10_000, Some(95))]);
        let r = report(&healthy, 100, &policy(2), 100);
        assert!(!r.at_risk && r.state == State::TargetSatisfied);
        let expiring = item(&[(1, 120, Some(90)), (2, 10_000, Some(95))]);
        let r = report(&expiring, 100, &policy(2), 100);
        assert_eq!(
            (r.expiring_soon, r.at_risk, r.state),
            (1, true, State::TargetSatisfied)
        );
        // Observations older than two intervals are stale even though the state is unchanged.
        let stale = item(&[(1, 10_000, Some(90)), (2, 10_000, None)]);
        let r = report(&stale, 400, &policy(2), 100);
        assert_eq!((r.stale_observations, r.at_risk), (2, true));
        let never_checked = report(&item(&[(1, 10_000, None)]), 150, &policy(2), 100);
        assert_eq!(never_checked.stale_observations, 0);
    }

    // Names are the CR2SE state names.
    #[test]
    fn state_names_match_cr2se() {
        assert_eq!(State::NotRedundant.name(), "not redundant");
        assert_eq!(State::UnderReplicated.name(), "under-replicated");
        assert_eq!(State::TargetSatisfied.name(), "target satisfied");
    }
}
