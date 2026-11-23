//! Recovery of the hosted publication participant before new work is admitted.
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

/// A policy that leaves every prepared operation to the Services coordinator: none is committed or aborted here, each stays prepared and
/// locked, and the coordinator then delivers its recorded decisions (with their retained decision times) through its own resume step.
#[derive(Debug, Clone, Copy, Default)]
pub struct DeferToServices;

impl Decisions for DeferToServices {
    /// Always undecided, so the coordinator alone decides.
    fn decision(&self, _key: OperationKey) -> Result<Decision, Error> {
        tracing::trace!("Deferring a prepared hosted publication operation to the coordinator");
        Ok(Decision::Undecided)
    }
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
        tracing::trace!("Recovering hosted publication participants before admitting new work");
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
        tracing::debug!(?report, "hosted publication recovery finished");
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::{
        hosted_publications::{
            effect::{Effect, Outcome},
            ids::{PlacementId, RemovalId},
            journal::Journal,
            metadata::Kind,
            operation::RemoveRequest,
            participant::Status,
            provider::{Provider, tests::public},
            signing::tests::VectorPublisher,
            store::{Limits, Store},
            terms::tests::terms,
        },
        identity::api::IdentityId,
        messaging::{
            ports::FixedLimits,
            test_support::{FixedClock, RecordingIssuer, SequenceEntropy},
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
            "ts-hosted-recovery-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }
    const LIMITS: Limits = Limits {
        placements: 8,
        content_bytes: 1000,
    };

    // A participant over native state in `root` with its clock at `now`, whose release issuer records into `issuer`.
    fn open(root: &std::path::Path, now: u64, issuer: Arc<RecordingIssuer>) -> Participant {
        let provider = Provider::new(
            terms(),
            Arc::new(Store::open(&root.join("placements"), LIMITS).unwrap()),
            Arc::new(FixedClock::at(now)),
            Arc::new(SequenceEntropy::new(now)),
            Arc::new(FixedLimits::default()),
        )
        .with_release_policy(Arc::new(|payments: u64, _: u64| payments))
        .with_release_issuer(issuer);
        Participant::new(
            Arc::new(provider),
            Journal::open(&root.join("journal"), 32).unwrap(),
            Arc::new(SequenceEntropy::new(now + 1)),
        )
    }

    // A store effect for the vector publisher's `abc` page at sequence `n`.
    fn store_effect(id: u8) -> Effect {
        let publisher = VectorPublisher::new();
        let publication = public(&publisher, id, Kind::Page, u64::from(id), b"abc");
        Effect::Store {
            placement_id: PlacementId([id; 32]),
            publication: Box::new(publication),
            retention_seconds: 10,
            price: 17,
        }
    }

    // Recovery applies recorded commits (idempotently), releases recorded aborts, leaves undecided operations prepared and locked, and
    // admits new work only afterwards; running it again changes nothing.
    #[test]
    fn recovery_resolves_prepared_operations_from_decisions() {
        let root = dir("decide");
        let issuer = Arc::new(RecordingIssuer::default());
        let participant = open(&root, 1000, issuer.clone());
        participant.set_ready();
        let tokens: Vec<_> = (1..=3u8)
            .map(|n| participant.prepare(key(n), &store_effect(n)).unwrap())
            .collect();
        // The crash happened after key 1 was applied but before its journal entry changed.
        let Some(Effect::Store { .. }) = participant.prepared_effect(key(1)) else {
            panic!("prepared")
        };
        drop(participant);
        let participant = open(&root, 1005, issuer);
        assert!(!participant.is_ready());
        assert_eq!(
            participant.prepare(key(9), &store_effect(9)),
            Err(Error::Unavailable)
        );
        let decisions = Table(
            BTreeMap::from([(key(1), Decision::Commit), (key(2), Decision::Abort)]),
            false,
        );
        let report = participant.recover(&decisions).unwrap();
        assert_eq!(
            report,
            Report {
                committed: 1,
                aborted: 1,
                undecided: 1
            }
        );
        assert!(participant.is_ready());
        assert!(matches!(
            participant.status(key(1)),
            Status::Committed(outcome) if matches!(*outcome, Outcome::Stored { accepted_at: 1005, .. })
        ));
        assert_eq!(participant.status(key(2)), Status::Aborted);
        assert_eq!(participant.status(key(3)), Status::Prepared(tokens[2]));
        // The undecided store still holds its placement: a conflicting effect cannot prepare.
        assert_eq!(
            participant.prepare(key(4), &store_effect(3)),
            Err(Error::Conflict)
        );
        let again = participant.recover(&decisions).unwrap();
        assert_eq!(again.committed, 0);
        assert_eq!(again.undecided, 1);
        assert_eq!(participant.provider().store().records().len(), 1);
    }

    // A decision source that cannot be read stops recovery and keeps new work refused, until it can be read again.
    #[test]
    fn unreadable_decisions_keep_new_work_refused() {
        let root = dir("unreadable");
        let issuer = Arc::new(RecordingIssuer::default());
        let participant = open(&root, 1000, issuer.clone());
        participant.set_ready();
        participant.prepare(key(1), &store_effect(1)).unwrap();
        drop(participant);
        let participant = open(&root, 1001, issuer);
        assert_eq!(
            participant.recover(&Table(BTreeMap::new(), true)),
            Err(Error::Unavailable)
        );
        assert!(!participant.is_ready());
        let report = participant.recover(&DeferToServices).unwrap();
        assert_eq!(
            (report.committed, report.aborted, report.undecided),
            (0, 0, 1)
        );
        assert!(participant.is_ready());
    }

    // A removal whose credits were issued just before a crash is completed exactly once by recovery: the same operation key issues again
    // idempotently and the removal is recorded.
    #[test]
    fn a_crashed_removal_completes_once() {
        let root = dir("removal");
        let issuer = Arc::new(RecordingIssuer::default());
        let participant = open(&root, 1000, issuer.clone());
        participant.set_ready();
        let store_key = key(1);
        let token = participant.prepare(store_key, &store_effect(1)).unwrap();
        participant.commit(store_key, &token).unwrap();
        let removal = Effect::Remove {
            request: RemoveRequest {
                placement_id: PlacementId([1; 32]),
                removal_id: RemovalId([3; 32]),
                expected_expires_at: 1010,
                minimum_release_credits: 0,
            },
            release_credits: 5,
            price: 2,
        };
        let token = participant.prepare(key(2), &removal).unwrap();
        drop(participant);
        let participant = open(&root, 1004, issuer.clone());
        let report = participant
            .recover(&Table(BTreeMap::from([(key(2), Decision::Commit)]), false))
            .unwrap();
        assert_eq!(report.committed, 1);
        assert_eq!(issuer.issued().len(), 1);
        // Committing the same decision again returns the retained outcome without issuing more.
        assert!(participant.commit(key(2), &token).is_ok());
        assert_eq!(issuer.issued().len(), 1);
    }
}
