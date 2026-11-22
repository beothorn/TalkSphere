//! Scheduling of replica checks, renewals and expiry by the redundancy policy.
//!
//! Callers: the maintenance loop asks [`plan`] what is due, performs the paid operations over the
//! holders' live connections and reports back. The planner is a pure function of the book, the clock
//! and the policy, so it is tested without any network. Nothing is retried in a tight loop: an
//! operation that could not be completed is not attempted again for the policy's retry delay, and an
//! inconclusive or unreachable result never counts as proof that a copy is gone or that another exists.
use super::book::Item;
use crate::{composition::policy::Redundancy, modules::identity::api::IdentityId};
use std::time::{Duration, Instant};

/// Most paid operations planned in one pass, so one pass never becomes a burst.
pub const MAXIMUM_PAID_ACTIONS: usize = 4;
/// Most remembered failed attempts.
const MAXIMUM_ATTEMPTS: usize = 4096;

/// One thing the maintenance loop should do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// The retention ended: forget the replica so the item needs another copy.
    Expire {
        /// Item identifier.
        item: [u8; 16],
        /// Holder of the expired replica.
        provider: IdentityId,
    },
    /// The retention is running out: extend it.
    Renew {
        /// Item identifier.
        item: [u8; 16],
        /// Holder of the replica.
        provider: IdentityId,
        /// Lease at the holder.
        lease: [u8; 32],
    },
    /// The replica is due for a paid byte check.
    Check {
        /// Item identifier.
        item: [u8; 16],
        /// Holder of the replica.
        provider: IdentityId,
        /// Lease at the holder.
        lease: [u8; 32],
        /// Size of the sealed bytes the holder stores, bounding the sampled offset.
        sealed_size: u64,
    },
}

/// Plan the work due at Unix time `now`: every expiry, then at most `paid_limit` renewals (soonest
/// expiry first) and checks (longest overdue first); the running node passes [`MAXIMUM_PAID_ACTIONS`].
/// `recently` says whether an operation on that replica was attempted so recently that it must wait.
pub fn plan(
    items: &[Item],
    now: u64,
    policy: &dyn Redundancy,
    recently: &dyn Fn([u8; 16], IdentityId) -> bool,
    paid_limit: usize,
) -> Vec<Action> {
    tracing::trace!("Planning replica maintenance");
    let mut expiries = Vec::new();
    let mut renewals = Vec::new();
    let mut checks = Vec::new();
    let mut sequence = 0usize;
    for item in items {
        for replica in &item.replicas {
            // One running index over all replicas, so checks of different replicas are staggered.
            let index = sequence;
            sequence += 1;
            if replica.expires_at <= now {
                expiries.push(Action::Expire {
                    item: item.id,
                    provider: replica.provider,
                });
            } else if recently(item.id, replica.provider) {
            } else if replica.expires_at - now <= policy.renew_before_seconds() {
                renewals.push((
                    replica.expires_at,
                    Action::Renew {
                        item: item.id,
                        provider: replica.provider,
                        lease: replica.lease,
                    },
                ));
            } else {
                let last = replica.last_checked.unwrap_or(replica.confirmed_at);
                let due = policy.next_check(last, index);
                if now >= due {
                    checks.push((
                        due,
                        Action::Check {
                            item: item.id,
                            provider: replica.provider,
                            lease: replica.lease,
                            sealed_size: item.sealed_size,
                        },
                    ));
                }
            }
        }
    }
    renewals.sort_by_key(|(expires, _)| *expires);
    checks.sort_by_key(|(due, _)| *due);
    let mut actions = expiries;
    actions.extend(
        renewals
            .into_iter()
            .map(|(_, a)| a)
            .chain(checks.into_iter().map(|(_, a)| a))
            .take(paid_limit),
    );
    actions
}

/// Remembers operations that could not be completed so they wait for the retry delay.
#[derive(Debug, Default)]
pub struct Attempts {
    failed: Vec<(([u8; 16], IdentityId), Instant)>,
}

impl Attempts {
    /// Note that an operation on this replica could not be completed just now.
    pub fn note(&mut self, item: [u8; 16], provider: IdentityId, now: Instant) {
        tracing::trace!("Remembering an incomplete maintenance attempt");
        self.failed.retain(|(key, _)| *key != (item, provider));
        if self.failed.len() >= MAXIMUM_ATTEMPTS {
            self.failed.remove(0);
        }
        self.failed.push(((item, provider), now));
    }

    /// Whether an operation on this replica was attempted less than `retry` ago.
    pub fn recent(
        &self,
        item: [u8; 16],
        provider: IdentityId,
        now: Instant,
        retry: Duration,
    ) -> bool {
        tracing::trace!("Checking for a recent maintenance attempt");
        self.failed
            .iter()
            .any(|(key, at)| *key == (item, provider) && now.duration_since(*at) < retry)
    }

    /// Forget a replica's attempt after it succeeded or was dropped.
    pub fn clear(&mut self, item: [u8; 16], provider: IdentityId) {
        tracing::trace!("Clearing a maintenance attempt");
        self.failed.retain(|(key, _)| *key != (item, provider));
    }
}

/// The background loop that performs due maintenance once a second until it is dropped.
pub struct Maintenance {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Maintenance {
    /// Start the loop; `pass` performs one maintenance pass with the loop's remembered attempts.
    pub fn start(mut pass: impl FnMut(&mut Attempts) + Send + 'static) -> Self {
        tracing::trace!("Starting the replica maintenance loop");
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::spawn(move || {
            let mut attempts = Attempts::default();
            while !flag.load(std::sync::atomic::Ordering::SeqCst) {
                pass(&mut attempts);
                // Sleep in short steps so a stop request is honored promptly.
                for _ in 0..10 {
                    if flag.load(std::sync::atomic::Ordering::SeqCst) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        });
        Self {
            stop,
            thread: Some(thread),
        }
    }
}

impl Drop for Maintenance {
    // Stop the loop and wait for the pass in progress to finish.
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composition::node::replicas::book::Replica;
    use crate::composition::policy::{Configured, Params};

    // A policy with a 1000-second check interval, 50 % stagger and renewal within 100 seconds of expiry.
    fn policy() -> Configured {
        Configured::new(
            Params::parse(
                br#"{"version":1,"redundancy":{"checkSeconds":1000,"staggerPercent":50,"renewBeforeSeconds":100}}"#,
            )
            .unwrap(),
        )
    }

    // An item with replicas (provider, expires_at, confirmed_at, last_checked).
    fn item(id: u8, replicas: &[(u8, u64, u64, Option<u64>)]) -> Item {
        Item {
            id: [id; 16],
            size: 3,
            sealed_size: 44,
            plaintext_hash: [0; 32],
            created_at: 0,
            replicas: replicas
                .iter()
                .map(|(p, expires_at, confirmed_at, last_checked)| Replica {
                    provider: IdentityId([*p; 32]),
                    lease: [*p; 32],
                    expires_at: *expires_at,
                    confirmed_at: *confirmed_at,
                    last_checked: *last_checked,
                })
                .collect(),
        }
    }

    // Nothing is attempted recently unless a test says so.
    fn never(_: [u8; 16], _: IdentityId) -> bool {
        false
    }

    // Expired replicas are dropped, nearly expired ones renewed, others left until their check is due.
    #[test]
    fn expiry_renewal_and_checks_are_told_apart() {
        let items = [item(
            1,
            &[(1, 500, 0, None), (2, 1050, 0, None), (3, 5000, 0, None)],
        )];
        let actions = plan(&items, 1000, &policy(), &never, MAXIMUM_PAID_ACTIONS);
        assert_eq!(
            actions[0],
            Action::Expire {
                item: [1; 16],
                provider: IdentityId([1; 32])
            }
        );
        assert_eq!(
            actions[1],
            Action::Renew {
                item: [1; 16],
                provider: IdentityId([2; 32]),
                lease: [2; 32]
            }
        );
        // Replica 3 was confirmed at 0, so its check came due at 1000 plus its stagger.
        let due = policy().next_check(0, 2);
        assert!(
            plan(&items, due - 1, &policy(), &never, MAXIMUM_PAID_ACTIONS)
                .iter()
                .all(|a| !matches!(a, Action::Check { .. }))
        );
        assert!(plan(&items, due, &policy(), &never, MAXIMUM_PAID_ACTIONS).iter().any(
            |a| matches!(a, Action::Check { provider, .. } if *provider == IdentityId([3; 32]))
        ));
    }

    // Checks of different replicas fall due at different times, so they are never all planned together:
    // with nothing performed in between, one more becomes due every 50 seconds, up to the pass bound.
    #[test]
    fn checks_are_staggered_across_replicas() {
        let replicas: Vec<(u8, u64, u64, Option<u64>)> =
            (1..=6).map(|p| (p, 100_000, 0, None)).collect();
        let items = [item(1, &replicas)];
        let due_at = |now| plan(&items, now, &policy(), &never, MAXIMUM_PAID_ACTIONS).len();
        let counts: Vec<usize> = [999, 1000, 1050, 1100, 1150, 1200]
            .iter()
            .map(|now| due_at(*now))
            .collect();
        assert_eq!(
            counts,
            vec![0, 1, 2, 3, MAXIMUM_PAID_ACTIONS, MAXIMUM_PAID_ACTIONS]
        );
        let distinct: std::collections::BTreeSet<u64> =
            (0..6).map(|i| policy().next_check(0, i)).collect();
        assert_eq!(distinct.len(), 6);
    }

    // At most the bound of paid operations is planned per pass, renewals before checks, soonest first.
    #[test]
    fn a_pass_is_bounded_and_urgent_work_goes_first() {
        let replicas: Vec<(u8, u64, u64, Option<u64>)> = (1..=10)
            .map(|p| (p, 1000 + u64::from(p), 0, None))
            .collect();
        let items = [item(1, &replicas)];
        let actions = plan(&items, 950, &policy(), &never, MAXIMUM_PAID_ACTIONS);
        assert_eq!(actions.len(), MAXIMUM_PAID_ACTIONS);
        let providers: Vec<u8> = actions
            .iter()
            .map(|a| match a {
                Action::Renew { provider, .. } => provider.0[0],
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(providers, vec![1, 2, 3, 4]);
    }

    // A replica attempted recently waits, an overdue check uses the last conclusive check as its base,
    // and an empty book plans nothing.
    #[test]
    fn recent_attempts_wait_and_checks_use_the_last_check() {
        let items = [item(1, &[(1, 100_000, 0, Some(9000))])];
        assert!(plan(&items, 9500, &policy(), &never, MAXIMUM_PAID_ACTIONS).is_empty());
        assert_eq!(
            plan(&items, 11_000, &policy(), &never, MAXIMUM_PAID_ACTIONS).len(),
            1
        );
        assert!(
            plan(
                &items,
                11_000,
                &policy(),
                &|_, _| true,
                MAXIMUM_PAID_ACTIONS
            )
            .is_empty()
        );
        assert!(plan(&[], 11_000, &policy(), &never, MAXIMUM_PAID_ACTIONS).is_empty());
        // An expiry is never postponed by a recent attempt: the retention has ended either way.
        let expired = [item(2, &[(1, 10, 0, None)])];
        assert_eq!(
            plan(&expired, 11, &policy(), &|_, _| true, MAXIMUM_PAID_ACTIONS).len(),
            1
        );
    }

    // The loop runs passes until dropped and stops promptly, even in the middle of its sleep.
    #[test]
    fn the_loop_runs_and_stops() {
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let seen = count.clone();
        let started = Instant::now();
        let loop_ = Maintenance::start(move |_| {
            seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        while count.load(std::sync::atomic::Ordering::SeqCst) == 0 {
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(loop_);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "the loop did not stop promptly"
        );
        let after = count.load(std::sync::atomic::Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(1200));
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), after);
    }

    // Attempts wait for the retry delay, can be cleared, and the memory is bounded.
    #[test]
    fn attempts_expire_and_are_bounded() {
        let start = Instant::now();
        let mut attempts = Attempts::default();
        let p = IdentityId([1; 32]);
        attempts.note([1; 16], p, start);
        let retry = Duration::from_secs(60);
        assert!(attempts.recent([1; 16], p, start + Duration::from_secs(59), retry));
        assert!(!attempts.recent([1; 16], p, start + Duration::from_secs(60), retry));
        assert!(!attempts.recent([2; 16], p, start, retry));
        attempts.clear([1; 16], p);
        assert!(!attempts.recent([1; 16], p, start, retry));
        for n in 0..(MAXIMUM_ATTEMPTS + 10) {
            let mut id = [0u8; 16];
            id[..8].copy_from_slice(&(n as u64).to_be_bytes());
            attempts.note(id, p, start);
        }
        assert_eq!(attempts.failed.len(), MAXIMUM_ATTEMPTS);
    }
}
