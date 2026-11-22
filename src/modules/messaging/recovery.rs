//! Recovery of the participant before new work is admitted.
//!
//! Callers: node composition at startup, after the Services coordinator has recovered its own decisions. Each
//! prepared operation found in the journal is resolved from the coordinator's durable decision: a recorded commit is
//! applied (idempotently, so an effect that was applied just before a crash is recognized), a recorded abort is
//! released, and an undecided operation stays prepared with its placement locked. New work is admitted only after every
//! decision could be consulted, so a store never races a half-finished operation, and nothing is ever re-executed or
//! retried on its own authority.
use super::{error::Error, participant::Participant};
use crate::modules::services::contract::OperationKey;

/// What the coordinator durably decided for an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// The coordinator recorded a commit.
    Commit,
    /// The coordinator recorded an abort.
    Abort,
    /// No decision has been recorded yet.
    Undecided,
}

/// The coordinator's recorded decisions, supplied by composition.
pub trait Decisions: Send + Sync {
    /// The durable decision for `key`.
    /// # Errors
    /// `Unavailable` when the decision cannot be read; recovery then stops and new work stays refused.
    fn decision(&self, key: OperationKey) -> Result<Decision, Error>;
}

/// What recovery did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Report {
    /// Operations committed.
    pub committed: usize,
    /// Operations aborted.
    pub aborted: usize,
    /// Operations left prepared because the coordinator has not decided.
    pub undecided: usize,
}

impl Participant {
    /// Resolve every prepared operation from the coordinator's decisions, then admit new work.
    /// # Errors
    /// A decision that cannot be read or an effect that cannot be applied stops recovery with that error; new work stays refused and
    /// recovery can be run again.
    pub fn recover(&self, decisions: &dyn Decisions) -> Result<Report, Error> {
        tracing::trace!("Recovering Messaging participants before admitting new work");
        let mut report = Report::default();
        for (key, token) in self.prepared() {
            match decisions.decision(key)? {
                Decision::Commit => {
                    self.commit(key, &token)?;
                    report.committed += 1;
                }
                Decision::Abort => {
                    self.abort(key, Some(&token))?;
                    report.aborted += 1;
                }
                Decision::Undecided => report.undecided += 1,
            }
        }
        self.set_ready();
        tracing::debug!(?report, "Messaging recovery finished");
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::{
        identity::api::IdentityId,
        messaging::{
            effect::{Effect, Outcome},
            ids::{MessageId, PlacementId},
            journal::Journal,
            participant::Status,
            placement::{Placement, Record},
            ports::Signer,
            signing,
            state::{Encoding, State},
            store::{Limits, Store},
            test_support::{FixedClock, SequenceEntropy, TestIdentity},
        },
    };
    use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

    // Decisions from a table; unknown keys are undecided, and the whole source can fail.
    struct Table(BTreeMap<OperationKey, Decision>, bool);
    impl Decisions for Table {
        fn decision(&self, key: OperationKey) -> Result<Decision, Error> {
            if self.1 {
                return Err(Error::Unavailable);
            }
            Ok(self.0.get(&key).copied().unwrap_or(Decision::Undecided))
        }
    }

    fn key(n: u8) -> OperationKey {
        OperationKey {
            requester: IdentityId([n; 32]),
            connection: [n; 32],
            stream: u32::from(n),
        }
    }
    fn dir(label: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static NEXT: AtomicU32 = AtomicU32::new(0);
        std::env::temp_dir().join(format!(
            "ts-messaging-recovery-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }
    const LIMITS: Limits = Limits {
        placements: 8,
        payload_bytes: 1000,
    };

    // A participant over native state in `root`, with its clock at `now`.
    fn open(root: &std::path::Path, now: u64) -> Participant {
        Participant::new(
            Arc::new(Store::open(&root.join("placements"), LIMITS).unwrap()),
            Journal::open(&root.join("journal"), 32).unwrap(),
            Arc::new(SequenceEntropy::new(now)),
            Arc::new(FixedClock::at(now)),
            Arc::new(TestIdentity::new(2)),
        )
    }
    fn store_effect(id: u8) -> Effect {
        let message = signing::sign_message(
            &TestIdentity::new(1),
            TestIdentity::new(3).identity(),
            MessageId([id; 32]),
            1,
            Encoding::Opaque,
            b"abc".to_vec(),
        )
        .unwrap();
        Effect::Store {
            placement_id: PlacementId([id; 32]),
            message,
            retention_seconds: 10,
            price: 17,
        }
    }

    // A new participant refuses work until recovery has run, whatever the journal holds.
    #[test]
    fn work_is_refused_until_recovery_completes() {
        let root = dir("fresh");
        let p = open(&root, 1000);
        assert_eq!(p.prepare(key(1), &store_effect(1)), Err(Error::Unavailable));
        assert_eq!(
            p.recover(&Table(BTreeMap::new(), false)),
            Ok(Report::default())
        );
        assert!(p.prepare(key(1), &store_effect(1)).is_ok());
    }

    // After a restart, prepared operations are resolved by the coordinator's decisions: a recorded commit takes effect once,
    // a recorded abort leaves nothing, and an undecided one stays prepared with its placement locked.
    #[test]
    fn decisions_resolve_prepared_operations_after_restart() {
        let root = dir("restart");
        let p = open(&root, 1000);
        p.recover(&Table(BTreeMap::new(), false)).unwrap();
        let tokens: Vec<_> = (1..=3)
            .map(|n| p.prepare(key(n), &store_effect(n)).unwrap())
            .collect();
        drop(p);
        // The node restarts later; nothing was committed before the crash.
        let p = open(&root, 1500);
        let decisions = Table(
            BTreeMap::from([(key(1), Decision::Commit), (key(2), Decision::Abort)]),
            false,
        );
        assert_eq!(
            p.recover(&decisions),
            Ok(Report {
                committed: 1,
                aborted: 1,
                undecided: 1
            })
        );
        let Status::Committed(outcome) = p.status(key(1)) else {
            panic!("committed expected")
        };
        assert_eq!(
            *outcome,
            Outcome::Stored {
                placement_id: PlacementId([1; 32]),
                message_id: MessageId([1; 32]),
                accepted_at: 1500,
                expires_at: 1510,
                price: 17
            }
        );
        assert_eq!(p.status(key(2)), Status::Aborted);
        assert_eq!(p.status(key(3)), Status::Prepared(tokens[2]));
        assert_eq!(
            p.store().record(PlacementId([1; 32])).unwrap().state,
            State::Pending
        );
        assert!(
            p.store().record(PlacementId([2; 32])).is_none()
                && p.store().record(PlacementId([3; 32])).is_none()
        );
        // The undecided store still holds its placement ID: no other operation may take it.
        assert_eq!(p.prepare(key(9), &store_effect(3)), Err(Error::Conflict));
        // Recovery is repeatable and applies nothing twice.
        assert_eq!(
            p.recover(&decisions),
            Ok(Report {
                committed: 0,
                aborted: 0,
                undecided: 1
            })
        );
        assert_eq!(p.store().records().len(), 1);
    }

    // The crash window after an effect was applied but before the journal recorded it: recovery recognizes the applied effect, keeps the
    // original acceptance time, and creates no second placement.
    #[test]
    fn an_effect_applied_before_the_crash_is_recognized() {
        let root = dir("window");
        let p = open(&root, 1000);
        p.recover(&Table(BTreeMap::new(), false)).unwrap();
        let effect = store_effect(1);
        p.prepare(key(1), &effect).unwrap();
        // The effect is applied exactly as commit would apply it, but the crash comes before the journal records the commit.
        let Effect::Store {
            placement_id,
            message,
            ..
        } = &effect
        else {
            unreachable!()
        };
        p.store()
            .insert(&Placement {
                record: Record {
                    id: *placement_id,
                    metadata: message.metadata.clone(),
                    sender_public_key: message.sender_public_key,
                    sender_signature: message.sender_signature,
                    accepted_at: 1000,
                    expires_at: 1010,
                    state: State::Pending,
                    payments: 17,
                    recovery_price: 0,
                    receipt: None,
                    removal: None,
                },
                payload: message.payload.clone(),
            })
            .unwrap();
        drop(p);
        let p = open(&root, 5000);
        let report = p
            .recover(&Table(BTreeMap::from([(key(1), Decision::Commit)]), false))
            .unwrap();
        assert_eq!(report.committed, 1);
        let record = p.store().record(PlacementId([1; 32])).unwrap();
        assert_eq!(
            (record.accepted_at, record.expires_at),
            (1000, 1010),
            "the original acceptance time is kept"
        );
        assert_eq!(p.store().records().len(), 1);
    }

    // When a decision cannot be read, recovery stops with an error and new work stays refused until a later run succeeds.
    #[test]
    fn an_unreadable_decision_keeps_work_refused() {
        let root = dir("unreadable");
        let p = open(&root, 1000);
        p.recover(&Table(BTreeMap::new(), false)).unwrap();
        p.prepare(key(1), &store_effect(1)).unwrap();
        drop(p);
        let p = open(&root, 2000);
        assert_eq!(
            p.recover(&Table(BTreeMap::new(), true)),
            Err(Error::Unavailable)
        );
        assert!(!p.is_ready());
        assert_eq!(p.prepare(key(2), &store_effect(2)), Err(Error::Unavailable));
        assert!(p.recover(&Table(BTreeMap::new(), false)).is_ok());
        assert!(p.prepare(key(2), &store_effect(2)).is_ok());
    }
}
