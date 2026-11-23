//! Participant tests: preparation, locks, idempotent commit at a decision time, abort, status and failure paths.
use super::*;
use crate::modules::{
    hosted_publications::{
        ids::RemovalId,
        metadata::Kind,
        operation::RemoveRequest,
        provider::tests::public,
        signing::tests::VectorPublisher,
        store::{Limits, Store},
        terms::tests::terms,
    },
    messaging::{
        ports::{FixedLimits, Signer},
        test_support::{FixedClock, RecordingIssuer, SequenceEntropy},
    },
};

fn key(n: u8) -> OperationKey {
    OperationKey {
        requester: IdentityId([n; 32]),
        connection: [n; 32],
        stream: u32::from(n),
    }
}

// A ready participant over volatile state with the clock at 1000, and the handles tests drive.
fn ready() -> (Participant, Arc<FixedClock>, Arc<RecordingIssuer>) {
    let clock = Arc::new(FixedClock::at(1000));
    let issuer = Arc::new(RecordingIssuer::default());
    let provider = Provider::new(
        terms(),
        Arc::new(Store::volatile(Limits {
            placements: 8,
            content_bytes: 1000,
        })),
        clock.clone(),
        Arc::new(SequenceEntropy::new(10)),
        Arc::new(FixedLimits::default()),
    )
    .with_release_policy(Arc::new(|payments: u64, _: u64| payments))
    .with_release_issuer(issuer.clone());
    let participant = Participant::new(
        Arc::new(provider),
        Journal::volatile(16),
        Arc::new(SequenceEntropy::new(500)),
    );
    participant.set_ready();
    (participant, clock, issuer)
}

// A store effect of `abc` as a page (or other kind) with the given IDs.
fn store_effect(placement: u8, publication: u8, kind: Kind, sequence: u64) -> Effect {
    let publisher = VectorPublisher::new();
    Effect::Store {
        placement_id: PlacementId([placement; 32]),
        publication: Box::new(public(&publisher, publication, kind, sequence, b"abc")),
        retention_seconds: 10,
        price: 17,
    }
}

fn renew(placement: u8, expected: u64) -> Effect {
    Effect::Renew {
        placement_id: PlacementId([placement; 32]),
        expected_expires_at: expected,
        additional_seconds: 10,
        ceiling_expires_at: 1200,
        price: 15,
    }
}

fn remove(placement: u8, expected: u64) -> Effect {
    Effect::Remove {
        request: RemoveRequest {
            placement_id: PlacementId([placement; 32]),
            removal_id: RemovalId([7; 32]),
            expected_expires_at: expected,
            minimum_release_credits: 0,
        },
        release_credits: 5,
        price: 2,
    }
}

// Preparing is idempotent for the same effect, a different effect under the key is a conflict, and a participant refuses new work until
// recovery admitted it.
#[test]
fn preparing_is_idempotent_and_waits_for_recovery() {
    let (p, _, _) = ready();
    let effect = store_effect(1, 1, Kind::Page, 1);
    let token = p.prepare(key(1), &effect).unwrap();
    assert_eq!(p.prepare(key(1), &effect), Ok(token));
    assert_eq!(p.status(key(1)), Status::Prepared(token));
    assert_eq!(p.prepared(), vec![(key(1), token)]);
    assert_eq!(p.prepared_effect(key(1)), Some(effect));
    assert_eq!(
        p.prepare(key(1), &store_effect(2, 2, Kind::Page, 2)),
        Err(Error::Conflict)
    );
    assert_eq!(p.status(key(9)), Status::Absent);
    let fresh = Participant::new(
        p.provider().clone(),
        Journal::volatile(4),
        Arc::new(SequenceEntropy::new(1)),
    );
    assert_eq!(
        fresh.prepare(key(1), &store_effect(1, 1, Kind::Page, 1)),
        Err(Error::Unavailable)
    );
}

// Locks: a second operation on the same placement, the same publication or the same page sequence cannot prepare until the first ends;
// resources never take a sequence slot, and aborting frees everything.
#[test]
fn conflicting_operations_cannot_both_prepare() {
    let (p, _, _) = ready();
    let token = p
        .prepare(key(1), &store_effect(1, 1, Kind::Page, 5))
        .unwrap();
    // Same placement ID, same publication ID, same kind and sequence: each conflicts with the held locks.
    assert_eq!(
        p.prepare(key(2), &store_effect(1, 2, Kind::Page, 6)),
        Err(Error::Conflict)
    );
    assert_eq!(
        p.prepare(key(2), &store_effect(2, 1, Kind::Page, 6)),
        Err(Error::Conflict)
    );
    assert_eq!(
        p.prepare(key(2), &store_effect(2, 2, Kind::Page, 5)),
        Err(Error::Conflict)
    );
    // Another kind, sequence and resources are independent.
    p.prepare(key(3), &store_effect(3, 3, Kind::FeedPost, 5))
        .unwrap();
    p.prepare(key(4), &store_effect(4, 4, Kind::Resource, 0))
        .unwrap();
    p.prepare(key(5), &store_effect(5, 5, Kind::Resource, 0))
        .unwrap();
    p.abort(key(1), Some(&token)).unwrap();
    p.prepare(key(2), &store_effect(2, 2, Kind::Page, 5))
        .unwrap();
}

// Commit applies once at the retained decision time, retains the outcome, and committing again (even much later or with the clock moved)
// returns the same outcome; a wrong token, unknown key and aborted operation are refused.
#[test]
fn commit_applies_once_at_the_decision_time() {
    let (p, clock, _) = ready();
    let token = p
        .prepare(key(1), &store_effect(1, 1, Kind::Page, 1))
        .unwrap();
    assert_eq!(
        p.commit_at(key(1), &[0; 32], 1000),
        Err(Error::NotAvailable)
    );
    assert_eq!(p.commit_at(key(8), &token, 1000), Err(Error::NotAvailable));
    clock.advance(40);
    let outcome = p.commit_at(key(1), &token, 1003).unwrap();
    assert_eq!(
        outcome,
        Outcome::Stored {
            placement_id: PlacementId([1; 32]),
            publication_id: PublicationId([1; 32]),
            accepted_at: 1003,
            expires_at: 1013,
            price: 17,
        }
    );
    assert_eq!(p.commit(key(1), &token), Ok(outcome.clone()));
    assert_eq!(p.status(key(1)), Status::Committed(Box::new(outcome)));
    assert_eq!(p.provider().store().records().len(), 1);
    let doomed = p
        .prepare(key(2), &store_effect(2, 2, Kind::Page, 2))
        .unwrap();
    p.abort(key(2), None).unwrap();
    p.abort(key(2), None).unwrap();
    assert_eq!(p.commit(key(2), &doomed), Err(Error::Conflict));
    assert_eq!(p.abort(key(1), None), Err(Error::Conflict));
    assert_eq!(p.provider().store().records().len(), 1);
    // A crash between applying and journaling is repaired by committing again at the original time.
    let again = p
        .prepare(key(3), &store_effect(3, 3, Kind::Page, 3))
        .unwrap();
    p.provider()
        .commit_store(
            &StorePlan {
                publisher: IdentityId(VectorPublisher::new().identity().0),
                publication: match p.prepared_effect(key(3)) {
                    Some(Effect::Store { publication, .. }) => *publication,
                    _ => panic!("prepared"),
                },
                retention_seconds: 10,
                price: 17,
            },
            PlacementId([3; 32]),
            1040,
        )
        .unwrap();
    clock.advance(500);
    let repaired = p.commit_at(key(3), &again, 1040).unwrap();
    assert!(matches!(
        repaired,
        Outcome::Stored {
            accepted_at: 1040,
            expires_at: 1050,
            ..
        }
    ));
    assert_eq!(p.provider().store().records().len(), 2);
}

// Preconditions are judged at prepare: renewing or removing an unknown, expired or stale placement, and storing over a full table,
// fail without leaving a prepared effect or a lock behind.
#[test]
fn preconditions_fail_without_leaving_locks() {
    let (p, clock, _) = ready();
    let token = p
        .prepare(key(1), &store_effect(1, 1, Kind::Page, 1))
        .unwrap();
    p.commit_at(key(1), &token, 1000).unwrap();
    assert_eq!(p.prepare(key(2), &renew(9, 1010)), Err(Error::NotAvailable));
    assert_eq!(p.prepare(key(2), &renew(1, 1011)), Err(Error::Conflict));
    assert_eq!(p.prepare(key(2), &remove(1, 1011)), Err(Error::Conflict));
    assert_eq!(
        p.prepare(key(2), &store_effect(2, 1, Kind::Page, 9)),
        Err(Error::Conflict)
    );
    assert_eq!(p.status(key(2)), Status::Absent);
    // The failed attempts left no lock: the valid renewal prepares.
    let renewal = p.prepare(key(2), &renew(1, 1010)).unwrap();
    // While it is prepared, a removal of the same placement cannot.
    assert_eq!(p.prepare(key(3), &remove(1, 1010)), Err(Error::Conflict));
    assert_eq!(
        p.commit_at(key(2), &renewal, 1001).unwrap(),
        Outcome::Renewed {
            placement_id: PlacementId([1; 32]),
            previous_expires_at: 1010,
            expires_at: 1020,
            price: 15,
        }
    );
    // The removal now prepares against the new expiry, and an expired placement is refused.
    p.prepare(key(3), &remove(1, 1020)).unwrap();
    clock.set(1020);
    assert_eq!(p.prepare(key(4), &renew(1, 1020)), Err(Error::Conflict));
    p.abort(key(3), None).unwrap();
    assert_eq!(p.prepare(key(5), &renew(1, 1020)), Err(Error::NotAvailable));
    let full = Participant::new(
        Arc::new(Provider::new(
            terms(),
            Arc::new(Store::volatile(Limits {
                placements: 0,
                content_bytes: 10,
            })),
            clock.clone(),
            Arc::new(SequenceEntropy::new(1)),
            Arc::new(FixedLimits::default()),
        )),
        Journal::volatile(4),
        Arc::new(SequenceEntropy::new(2)),
    );
    full.set_ready();
    assert_eq!(
        full.prepare(key(1), &store_effect(1, 1, Kind::Page, 1)),
        Err(Error::Limit)
    );
}

// A removal commits once through the Ledger port: the credits are issued under the operation key, a repeat commit issues nothing more, and
// a refused issuance leaves the effect prepared so committing again is safe.
#[test]
fn removal_releases_credits_through_the_issuer() {
    let (p, _, issuer) = ready();
    let token = p
        .prepare(key(1), &store_effect(1, 1, Kind::Page, 1))
        .unwrap();
    p.commit_at(key(1), &token, 1000).unwrap();
    let removal = p.prepare(key(2), &remove(1, 1010)).unwrap();
    issuer.fail_with(Some(crate::modules::messaging::Error::Insufficient));
    assert_eq!(
        p.commit_at(key(2), &removal, 1001),
        Err(Error::Insufficient)
    );
    assert_eq!(p.status(key(2)), Status::Prepared(removal));
    assert_eq!(
        p.provider().store().records()[0].state,
        super::super::placement::State::Pending
    );
    issuer.fail_with(None);
    let outcome = p.commit_at(key(2), &removal, 1001).unwrap();
    assert_eq!(
        outcome,
        Outcome::Removed {
            placement_id: PlacementId([1; 32]),
            removal_id: RemovalId([7; 32]),
            release_credits: 5,
            price: 2,
        }
    );
    assert_eq!(
        issuer.issued(),
        vec![(key(2), IdentityId(VectorPublisher::new().identity().0), 5)]
    );
    assert_eq!(p.commit(key(2), &removal), Ok(outcome));
    assert_eq!(issuer.issued().len(), 1);
}

// A journal write failure at prepare leaves no lock, and one after applying is repaired by committing again; a query effect needs no
// lock and is retained for a restart.
#[test]
fn write_failures_and_queries() {
    let clock = Arc::new(FixedClock::at(1000));
    let journal = Journal::volatile(8);
    let provider = Provider::new(
        terms(),
        Arc::new(Store::volatile(Limits {
            placements: 8,
            content_bytes: 1000,
        })),
        clock,
        Arc::new(SequenceEntropy::new(10)),
        Arc::new(FixedLimits::default()),
    );
    let p = Participant::new(
        Arc::new(provider),
        journal,
        Arc::new(SequenceEntropy::new(500)),
    );
    p.set_ready();
    p.journal.fail_writes(true);
    assert_eq!(
        p.prepare(key(1), &store_effect(1, 1, Kind::Page, 1)),
        Err(Error::Storage)
    );
    p.journal.fail_writes(false);
    let token = p
        .prepare(key(1), &store_effect(1, 1, Kind::Page, 1))
        .unwrap();
    p.journal.fail_writes(true);
    assert_eq!(p.commit_at(key(1), &token, 1000), Err(Error::Storage));
    p.journal.fail_writes(false);
    assert!(matches!(
        p.commit_at(key(1), &token, 1000),
        Ok(Outcome::Stored {
            accepted_at: 1000,
            ..
        })
    ));
    assert_eq!(p.provider().store().records().len(), 1);
    let query = p.stage_query(key(2), vec![1, 2, 3]).unwrap();
    assert_eq!(
        p.prepared_effect(key(2)),
        Some(Effect::Query {
            request: vec![1, 2, 3]
        })
    );
    assert_eq!(p.commit(key(2), &query), Ok(Outcome::Query));
}
