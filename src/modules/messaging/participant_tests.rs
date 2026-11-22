//! Participant tests: preparation, idempotent commit, abort, status, locks, commit-time timestamps and failure windows.
use super::*;
use crate::modules::{
    identity::api::IdentityId,
    messaging::{
        ids::{MessageId, RecoveryId},
        operation::RemoveInput,
        signing,
        state::Encoding,
        store::Limits,
        test_support::{FixedClock, RecordingIssuer, SequenceEntropy, TestIdentity},
    },
};

struct Fixture {
    participant: Participant,
    clock: Arc<FixedClock>,
    store: Arc<Store>,
    provider: Arc<TestIdentity>,
    issuer: Arc<RecordingIssuer>,
}

// A ready participant over volatile state with the clock at 1000.
fn fixture() -> Fixture {
    let clock = Arc::new(FixedClock::at(1000));
    let store = Arc::new(Store::volatile(Limits {
        placements: 8,
        payload_bytes: 1000,
    }));
    let provider = Arc::new(TestIdentity::new(2));
    let issuer = Arc::new(RecordingIssuer::default());
    let participant = Participant::new(
        store.clone(),
        Journal::volatile(64),
        Arc::new(SequenceEntropy::new(1)),
        clock.clone(),
        provider.clone(),
    )
    .with_release_issuer(issuer.clone());
    participant.set_ready();
    Fixture {
        participant,
        clock,
        store,
        provider,
        issuer,
    }
}
fn key(n: u8) -> OperationKey {
    OperationKey {
        requester: IdentityId([n; 32]),
        connection: [n; 32],
        stream: u32::from(n),
    }
}
// A real signed message from A (seed 1) to C (seed 3).
fn message() -> super::super::metadata::SignedMessage {
    signing::sign_message(
        &TestIdentity::new(1),
        TestIdentity::new(3).identity(),
        MessageId([1; 32]),
        1000,
        Encoding::Opaque,
        b"abc".to_vec(),
    )
    .unwrap()
}
fn store_effect(f: &Fixture, retention: u64) -> Effect {
    Effect::Store {
        placement_id: f.participant.new_placement_id().unwrap(),
        message: message(),
        retention_seconds: retention,
        price: 17,
    }
}
// Store a message through prepare and commit and return its placement.
fn stored(f: &Fixture) -> PlacementId {
    let effect = store_effect(f, 10);
    let id = effect.placement().unwrap();
    let token = f.participant.prepare(key(1), &effect).unwrap();
    f.participant.commit(key(1), &token).unwrap();
    id
}
fn recover_effect(id: PlacementId, recovery: u8) -> Effect {
    let m = message();
    let provider = TestIdentity::new(2);
    let input = receipt::sign_request(
        &TestIdentity::new(3),
        provider.identity(),
        id,
        m.metadata.message_id,
        m.metadata.payload_hash,
        RecoveryId([recovery; 32]),
    )
    .unwrap();
    Effect::Recover { input, price: 2 }
}

// Nothing is admitted before recovery completes, and a prepared effect is invisible until it commits.
#[test]
fn work_waits_for_recovery_and_prepared_effects_are_invisible() {
    let clock = Arc::new(FixedClock::at(1000));
    let store = Arc::new(Store::volatile(Limits {
        placements: 8,
        payload_bytes: 1000,
    }));
    let p = Participant::new(
        store.clone(),
        Journal::volatile(8),
        Arc::new(SequenceEntropy::new(1)),
        clock,
        Arc::new(TestIdentity::new(2)),
    );
    let effect = Effect::Store {
        placement_id: PlacementId([5; 32]),
        message: message(),
        retention_seconds: 10,
        price: 17,
    };
    assert_eq!(p.prepare(key(1), &effect), Err(Error::Unavailable));
    p.set_ready();
    p.prepare(key(1), &effect).unwrap();
    assert_eq!(
        store.record(PlacementId([5; 32])),
        None,
        "a prepared store creates nothing"
    );
}

// MS-02 timing: acceptedAt is taken at the durable commit, not at preparation, and the expiry is that time plus the retention.
#[test]
fn acceptance_time_is_assigned_at_commit() {
    let f = fixture();
    let effect = store_effect(&f, 10);
    let token = f.participant.prepare(key(1), &effect).unwrap();
    f.clock.advance(5);
    let Outcome::Stored {
        accepted_at,
        expires_at,
        price,
        ..
    } = f.participant.commit(key(1), &token).unwrap()
    else {
        panic!("stored expected")
    };
    assert_eq!((accepted_at, expires_at, price), (1005, 1015, 17));
    let record = f.store.record(effect.placement().unwrap()).unwrap();
    assert_eq!(
        (
            record.accepted_at,
            record.expires_at,
            record.state,
            record.payments
        ),
        (1005, 1015, State::Pending, 17)
    );
    assert_eq!(f.store.get(record.id).unwrap().unwrap().payload, b"abc");
}

// Preparing the same effect again returns the same token, a different effect under the key conflicts, and committing twice
// returns the retained outcome without a second placement.
#[test]
fn prepare_and_commit_are_idempotent() {
    let f = fixture();
    let effect = store_effect(&f, 10);
    let token = f.participant.prepare(key(1), &effect).unwrap();
    assert_eq!(f.participant.prepare(key(1), &effect), Ok(token));
    assert_eq!(
        f.participant.prepare(key(1), &store_effect(&f, 11)),
        Err(Error::Conflict)
    );
    let first = f.participant.commit(key(1), &token).unwrap();
    f.clock.advance(100);
    assert_eq!(f.participant.commit(key(1), &token), Ok(first.clone()));
    assert_eq!(f.store.records().len(), 1);
    assert_eq!(
        f.participant.status(key(1)),
        Status::Committed(Box::new(first))
    );
    assert_eq!(
        f.participant.prepare(key(1), &effect),
        Err(Error::Conflict),
        "a finished key cannot be prepared again"
    );
}

// Status follows the lifecycle; a wrong token or an unknown key cannot commit; abort releases a prepared effect, is idempotent, and
// cannot undo a commit.
#[test]
fn status_tokens_and_abort() {
    let f = fixture();
    assert_eq!(f.participant.status(key(1)), Status::Absent);
    let effect = store_effect(&f, 10);
    let token = f.participant.prepare(key(1), &effect).unwrap();
    assert_eq!(f.participant.status(key(1)), Status::Prepared(token));
    assert_eq!(
        f.participant.commit(key(1), &[0; 32]),
        Err(Error::NotAvailable)
    );
    assert_eq!(
        f.participant.commit(key(9), &token),
        Err(Error::NotAvailable)
    );
    assert_eq!(
        f.participant.abort(key(1), Some(&[0; 32])),
        Err(Error::NotAvailable)
    );
    assert_eq!(f.participant.abort(key(1), Some(&token)), Ok(()));
    assert_eq!(f.participant.abort(key(1), None), Ok(()));
    assert_eq!(f.participant.abort(key(9), None), Ok(()));
    assert_eq!(f.participant.status(key(1)), Status::Aborted);
    assert_eq!(f.participant.commit(key(1), &token), Err(Error::Conflict));
    assert_eq!(f.store.records().len(), 0);
    // A committed operation cannot be aborted.
    let effect = store_effect(&f, 10);
    let token = f.participant.prepare(key(5), &effect).unwrap();
    f.participant.commit(key(5), &token).unwrap();
    assert_eq!(f.participant.abort(key(5), None), Err(Error::Conflict));
}

// A placement with a prepared effect is locked: a second operation cannot prepare on it until the first commits or aborts.
#[test]
fn a_prepared_effect_locks_its_placement() {
    let f = fixture();
    let id = stored(&f);
    let first = f
        .participant
        .prepare(key(2), &recover_effect(id, 7))
        .unwrap();
    assert_eq!(
        f.participant.prepare(key(3), &recover_effect(id, 8)),
        Err(Error::Conflict)
    );
    let remove = Effect::Remove {
        input: RemoveInput {
            placement_id: id,
            removal_id: RemovalId([4; 32]),
            expected_expires_at: 1010,
            minimum_release_credits: 4,
        },
        release_credits: 5,
        price: 2,
    };
    assert_eq!(f.participant.prepare(key(4), &remove), Err(Error::Conflict));
    f.participant.abort(key(2), Some(&first)).unwrap();
    f.participant.prepare(key(4), &remove).unwrap();
    assert_eq!(
        f.participant.prepare(key(3), &recover_effect(id, 8)),
        Err(Error::Conflict)
    );
}

// MS-07: a recovery commit produces a receipt that verifies under the provider key, taken at the commit time.
#[test]
fn recovery_records_a_verifiable_receipt() {
    let f = fixture();
    let id = stored(&f);
    let effect = recover_effect(id, 7);
    let token = f.participant.prepare(key(2), &effect).unwrap();
    f.clock.advance(5);
    let Outcome::Recovered {
        receipt: r, price, ..
    } = f.participant.commit(key(2), &token).unwrap()
    else {
        panic!("recovered expected")
    };
    assert_eq!((r.recovered_at, price), (1005, 2));
    let record = f.store.record(id).unwrap();
    assert_eq!((record.state, record.receipt), (State::Recovered, Some(r)));
    let binding = Binding {
        provider_id: f.provider.identity(),
        placement_id: id,
        message_id: record.metadata.message_id,
        sender_id: record.metadata.sender_id,
        recipient_id: record.metadata.recipient_id,
        payload_hash: record.metadata.payload_hash,
    };
    assert_eq!(
        receipt::verify_receipt(&r, &binding, &f.provider.key()),
        Ok(())
    );
}

// Renewal and removal effects apply once, keep their outcomes, and removal releases the payload.
#[test]
fn renewal_and_removal_apply_once() {
    let f = fixture();
    let id = stored(&f);
    let renew = Effect::Renew {
        placement_id: id,
        expected_expires_at: 1010,
        additional_seconds: 4,
        ceiling_expires_at: 1100,
        price: 6,
    };
    let token = f.participant.prepare(key(2), &renew).unwrap();
    assert_eq!(
        f.participant.commit(key(2), &token),
        Ok(Outcome::Renewed {
            placement_id: id,
            previous_expires_at: 1010,
            expires_at: 1014,
            price: 6
        })
    );
    assert_eq!(f.store.record(id).unwrap().payments, 23);
    let remove = Effect::Remove {
        input: RemoveInput {
            placement_id: id,
            removal_id: RemovalId([4; 32]),
            expected_expires_at: 1014,
            minimum_release_credits: 4,
        },
        release_credits: 5,
        price: 2,
    };
    let token = f.participant.prepare(key(3), &remove).unwrap();
    assert_eq!(
        f.participant.commit(key(3), &token),
        Ok(Outcome::Removed {
            placement_id: id,
            removal_id: RemovalId([4; 32]),
            release_credits: 5,
            price: 2
        })
    );
    assert_eq!(f.store.record(id).unwrap().state, State::Removed);
    assert!(f.store.get(id).is_err(), "the payload is released");
    assert_eq!(
        f.issuer.issued(),
        vec![(key(3), f.store.record(id).unwrap().metadata.sender_id, 5)],
        "the release credits went to the sender once"
    );
}

// A removal that releases credits and one that cannot issue them: without an issuer it is Unsupported, with a failing issuer the
// placement stays pending and prepared, and committing again after the issuer recovers removes it and issues the credits once.
#[test]
fn removal_settles_release_credits_once_and_recoverably() {
    let f = fixture();
    let id = stored(&f);
    let remove = Effect::Remove {
        input: RemoveInput {
            placement_id: id,
            removal_id: RemovalId([4; 32]),
            expected_expires_at: 1010,
            minimum_release_credits: 4,
        },
        release_credits: 5,
        price: 2,
    };
    let token = f.participant.prepare(key(3), &remove).unwrap();
    f.issuer.fail_with(Some(Error::Insufficient));
    assert_eq!(
        f.participant.commit(key(3), &token),
        Err(Error::Insufficient)
    );
    assert_eq!(f.store.record(id).unwrap().state, State::Pending);
    assert!(f.issuer.issued().is_empty());
    // The issuer recovers, but the record cannot be written: the credits are issued, the removal is not recorded.
    f.issuer.fail_with(None);
    f.store.fail_writes(true);
    assert!(f.participant.commit(key(3), &token).is_err());
    assert_eq!(f.issuer.issued().len(), 1);
    assert_eq!(f.store.record(id).unwrap().state, State::Pending);
    // Committing again finishes the same removal; the credits are not issued a second time.
    f.store.fail_writes(false);
    assert!(matches!(
        f.participant.commit(key(3), &token),
        Ok(Outcome::Removed {
            release_credits: 5,
            ..
        })
    ));
    assert_eq!(
        f.issuer.issued(),
        vec![(key(3), f.store.record(id).unwrap().metadata.sender_id, 5)]
    );
    // Applying again (crash repair after the record was written but before the journal was) does not issue twice either.
    assert!(f.participant.apply(key(3), &remove).is_ok());
    assert_eq!(f.issuer.issued().len(), 1);
}

// Credits are never issued for a removal that is refused at commit: an expired placement releases nothing.
#[test]
fn an_expired_removal_issues_nothing() {
    let f = fixture();
    let id = stored(&f);
    let remove = Effect::Remove {
        input: RemoveInput {
            placement_id: id,
            removal_id: RemovalId([4; 32]),
            expected_expires_at: 1010,
            minimum_release_credits: 4,
        },
        release_credits: 5,
        price: 2,
    };
    let token = f.participant.prepare(key(3), &remove).unwrap();
    f.clock.advance(10);
    assert_eq!(
        f.participant.commit(key(3), &token),
        Err(Error::NotAvailable)
    );
    assert!(f.issuer.issued().is_empty());
}

// Without an issuer installed, a removal that releases credits is refused as unsupported and changes nothing.
#[test]
fn removal_without_an_issuer_is_unsupported() {
    let clock = Arc::new(FixedClock::at(1000));
    let store = Arc::new(Store::volatile(Limits {
        placements: 8,
        payload_bytes: 1000,
    }));
    let participant = Participant::new(
        store.clone(),
        Journal::volatile(8),
        Arc::new(SequenceEntropy::new(1)),
        clock.clone(),
        Arc::new(TestIdentity::new(2)),
    );
    participant.set_ready();
    let f = Fixture {
        participant,
        clock,
        store,
        provider: Arc::new(TestIdentity::new(2)),
        issuer: Arc::new(RecordingIssuer::default()),
    };
    let id = stored(&f);
    let remove = Effect::Remove {
        input: RemoveInput {
            placement_id: id,
            removal_id: RemovalId([4; 32]),
            expected_expires_at: 1010,
            minimum_release_credits: 4,
        },
        release_credits: 5,
        price: 2,
    };
    let token = f.participant.prepare(key(3), &remove).unwrap();
    assert_eq!(
        f.participant.commit(key(3), &token),
        Err(Error::Unsupported)
    );
    assert_eq!(f.store.record(id).unwrap().state, State::Pending);
}

// Preconditions are checked at prepare: unknown, expired or finished placements, stale expectations and a full store are refused
// before anything is journaled or locked.
#[test]
fn preconditions_fail_before_anything_is_prepared() {
    let f = fixture();
    let id = stored(&f);
    assert_eq!(
        f.participant
            .prepare(key(2), &recover_effect(PlacementId([9; 32]), 7)),
        Err(Error::NotAvailable)
    );
    let stale = Effect::Renew {
        placement_id: id,
        expected_expires_at: 1009,
        additional_seconds: 4,
        ceiling_expires_at: 1100,
        price: 6,
    };
    assert_eq!(f.participant.prepare(key(2), &stale), Err(Error::Conflict));
    assert_eq!(f.participant.status(key(2)), Status::Absent);
    f.clock.advance(10);
    assert_eq!(
        f.participant.prepare(key(2), &recover_effect(id, 7)),
        Err(Error::NotAvailable),
        "expired"
    );
    let small = Store::volatile(Limits {
        placements: 1,
        payload_bytes: 1000,
    });
    let p = Participant::new(
        Arc::new(small),
        Journal::volatile(8),
        Arc::new(SequenceEntropy::new(1)),
        Arc::new(FixedClock::at(1000)),
        Arc::new(TestIdentity::new(2)),
    );
    p.set_ready();
    let first = Effect::Store {
        placement_id: PlacementId([1; 32]),
        message: message(),
        retention_seconds: 10,
        price: 17,
    };
    let token = p.prepare(key(1), &first).unwrap();
    p.commit(key(1), &token).unwrap();
    let second = Effect::Store {
        placement_id: PlacementId([2; 32]),
        message: message(),
        retention_seconds: 10,
        price: 17,
    };
    assert_eq!(p.prepare(key(2), &second), Err(Error::Limit));
    assert_eq!(
        p.prepare(
            key(3),
            &Effect::Store {
                placement_id: PlacementId([1; 32]),
                message: message(),
                retention_seconds: 10,
                price: 17
            }
        ),
        Err(Error::Conflict)
    );
}

// A journal write that fails while preparing leaves nothing prepared or locked; one that fails after the effect was applied leaves the
// effect applied, and committing again completes it with the same outcome and no second placement.
#[test]
fn failure_windows_are_repaired_by_committing_again() {
    let clock = Arc::new(FixedClock::at(1000));
    let store = Arc::new(Store::volatile(Limits {
        placements: 8,
        payload_bytes: 1000,
    }));
    let journal = Journal::volatile(8);
    let p = Participant::new(
        store.clone(),
        journal,
        Arc::new(SequenceEntropy::new(1)),
        clock.clone(),
        Arc::new(TestIdentity::new(2)),
    );
    p.set_ready();
    let effect = Effect::Store {
        placement_id: PlacementId([5; 32]),
        message: message(),
        retention_seconds: 10,
        price: 17,
    };
    p.journal.fail_writes(true);
    assert_eq!(p.prepare(key(1), &effect), Err(Error::Storage));
    assert_eq!(p.status(key(1)), Status::Absent);
    p.journal.fail_writes(false);
    let token = p.prepare(key(1), &effect).unwrap();
    // The effect is applied but the journal cannot record it: the commit reports a storage failure, yet the placement exists.
    p.journal.fail_writes(true);
    assert_eq!(p.commit(key(1), &token), Err(Error::Storage));
    assert_eq!(store.records().len(), 1);
    assert_eq!(p.status(key(1)), Status::Prepared(token));
    clock.advance(50);
    p.journal.fail_writes(false);
    let Outcome::Stored {
        accepted_at,
        expires_at,
        ..
    } = p.commit(key(1), &token).unwrap()
    else {
        panic!("stored expected")
    };
    assert_eq!(
        (accepted_at, expires_at),
        (1000, 1010),
        "the original acceptance time is reported"
    );
    assert_eq!(store.records().len(), 1);
}

// A clock or entropy failure refuses new work cleanly, and the same outcome is never produced twice for a recovery.
#[test]
fn dependency_failures_refuse_work_and_recovery_applies_once() {
    let f = fixture();
    let id = stored(&f);
    let effect = recover_effect(id, 7);
    let token = f.participant.prepare(key(2), &effect).unwrap();
    let first = f.participant.commit(key(2), &token).unwrap();
    // Re-applying the same recovery (as crash repair would) returns the recorded receipt and does not sign again.
    assert_eq!(f.participant.apply(key(2), &effect), Ok(first));
    f.clock.fail();
    assert_eq!(
        f.participant.prepare(key(3), &Effect::Query),
        Err(Error::Unavailable)
    );
    f.clock.set(1000);
    assert!(f.participant.prepare(key(3), &Effect::Query).is_ok());
    f.provider.break_signing();
    let other = stored_other(&f);
    let again = f
        .participant
        .prepare(key(5), &recover_effect(other, 9))
        .unwrap();
    assert_eq!(
        f.participant.commit(key(5), &again),
        Err(Error::Unavailable),
        "the receipt cannot be signed"
    );
    assert_eq!(
        f.store.record(other).unwrap().state,
        State::Pending,
        "nothing changed"
    );
}

// A second pending placement for the signing-failure test.
fn stored_other(f: &Fixture) -> PlacementId {
    let effect = store_effect(f, 10);
    let id = effect.placement().unwrap();
    let token = f.participant.prepare(key(8), &effect).unwrap();
    f.participant.commit(key(8), &token).unwrap();
    id
}
