//! Real durable renewal results, competing guarded participants and failure/reply-loss recovery through owner admission.
use super::*;
use crate::modules::storage::{
    image::Stage,
    journal::Journal,
    requester::renew,
    test_support::{
        Write,
        mutations::{B, CryptoPort, Terminal, fixture, image_limits, time},
    },
};
use std::sync::Arc;
// Keep the original authenticated accepted quote while changing only the distinct connection-local attempt.
fn attempt(invocation: &Invocation, stream: u32) -> Invocation {
    Invocation {
        key: services::contract::OperationKey {
            stream,
            ..invocation.key
        },
        ..invocation.clone()
    }
}
// Exact staging leaves the lease unchanged; one acknowledged commit extends only once and survives native reopen.
#[test]
fn exact_result_commit_and_native_reopen() {
    let f = fixture();
    let original = f.journal.snapshot().unwrap().leases[0].clone();
    let completion = stage(&f.gate, &f.invocation, &f.input, time(1), None, B).unwrap();
    assert_eq!(completion.output, f.output);
    assert_eq!(completion.price, 15);
    assert_eq!(f.journal.snapshot().unwrap().leases[0].expires_at, 1010);
    let updated =
        renew::validate(&f.invocation, &f.input, &completion.output, &original, B).unwrap();
    assert_eq!(updated.expires_at, 1020);
    let token = f
        .gate
        .prepare(f.invocation.key, &completion.effect)
        .unwrap();
    f.gate.commit(f.invocation.key, &token).unwrap();
    f.gate.commit(f.invocation.key, &token).unwrap();
    assert_eq!(f.journal.snapshot().unwrap().leases[0].expires_at, 1020);
    // Local result recovery uses original scope even after the old guard became stale; no remote retry is created.
    assert_eq!(
        stage(&f.gate, &f.invocation, &f.input, time(11), None, B)
            .unwrap()
            .output,
        completion.output
    );
    let reopened = Arc::new(
        Journal::open(
            f.invocation.provider,
            f.faults.clone(),
            Arc::new(CryptoPort),
            image_limits(),
        )
        .unwrap(),
    );
    assert!(matches!(
        reopened.status(f.invocation.key),
        Ok(Some(Stage::Committed(_)))
    ));
    assert_eq!(
        reopened.completion(f.invocation.key).unwrap().output,
        completion.output
    );
    assert_eq!(reopened.snapshot().unwrap().leases[0].expires_at, 1020);
    assert_eq!(
        f.bytes.reopen([8; 32], 3).unwrap().read(0, 3, 3).unwrap(),
        b"abc"
    );
    assert!(f.path.0.exists());
    let mut changed = f.input.clone();
    if let Value::Object(fields) = &mut changed {
        fields
            .iter_mut()
            .find(|(n, _)| n == "additionalSeconds")
            .unwrap()
            .1 = Value::Uint64(11);
    }
    assert!(matches!(
        stage(&f.gate, &f.invocation, &changed, time(1), None, B),
        Err(Error::Conflict)
    ));
}
// Two genuine preparation threads for the same original expiry cannot both acquire mutation authority.
#[test]
fn competing_renewals_prepare_at_most_once() {
    let f = fixture();
    let other = attempt(&f.invocation, 3);
    let first = stage(&f.gate, &f.invocation, &f.input, time(1), None, B).unwrap();
    let second = stage(&f.gate, &other, &f.input, time(1), None, B).unwrap();
    let (a, b) = std::thread::scope(|scope| {
        let a = scope.spawn(|| f.gate.prepare(f.invocation.key, &first.effect));
        let b = scope.spawn(|| f.gate.prepare(other.key, &second.effect));
        (a.join().unwrap(), b.join().unwrap())
    });
    let (winner, token, loser, effect) = match (a, b) {
        (Ok(token), Err(Error::Conflict)) => (f.invocation.key, token, other.key, second.effect),
        (Err(Error::Conflict), Ok(token)) => (other.key, token, f.invocation.key, first.effect),
        _ => panic!("exactly one preparation must win"),
    };
    f.gate.commit(winner, &token).unwrap();
    assert!(matches!(
        f.gate.prepare(loser, &effect),
        Err(Error::Conflict)
    ));
    f.gate.abort(loser, None).unwrap();
    assert_eq!(f.journal.snapshot().unwrap().leases[0].expires_at, 1020);
}
// A timely accepted renewal remains an original proposal across the old expiry; new attempts cannot use expired metadata.
#[test]
fn expiry_while_pending_and_stale_new_attempts() {
    let f = fixture();
    let completion = stage(&f.gate, &f.invocation, &f.input, time(9), None, B).unwrap();
    let token = f
        .gate
        .prepare(f.invocation.key, &completion.effect)
        .unwrap();
    let other = attempt(&f.invocation, 3);
    assert!(matches!(
        stage(&f.gate, &other, &f.input, time(10), None, B),
        Err(Error::Unavailable)
    ));
    f.gate.commit(f.invocation.key, &token).unwrap();
    f.gate
        .recover(&Terminal(&f.journal), time(11), None)
        .unwrap();
    assert_eq!(
        f.journal.snapshot().unwrap().leases[0].state,
        super::super::contract::State::Active
    );
    assert!(matches!(
        stage(&f.gate, &other, &f.input, time(11), None, B),
        Err(Error::Conflict)
    ));
}
// Confirmed stage failure releases provisional capacity; uncertain writes close admission and require original decision recovery.
#[test]
fn persistence_failures_and_lost_commit_acknowledgement() {
    let f = fixture();
    f.faults.writes.lock().unwrap().push_back(Write::Fail);
    assert!(matches!(
        stage(&f.gate, &f.invocation, &f.input, time(1), None, B),
        Err(Error::Unavailable)
    ));
    assert!(f.gate.is_ready().unwrap());
    assert!(f.journal.status(f.invocation.key).unwrap().is_none());
    let c = stage(&f.gate, &f.invocation, &f.input, time(1), None, B).unwrap();
    let token = f.gate.prepare(f.invocation.key, &c.effect).unwrap();
    f.faults.writes.lock().unwrap().push_back(Write::LoseReply);
    assert!(matches!(
        f.gate.commit(f.invocation.key, &token),
        Err(Error::Unknown)
    ));
    assert!(!f.gate.is_ready().unwrap());
    assert!(matches!(f.journal.snapshot(), Err(Error::Unknown)));
    f.journal.reload().unwrap();
    f.gate.commit(f.invocation.key, &token).unwrap();
    f.gate
        .recover(&Terminal(&f.journal), time(2), None)
        .unwrap();
    assert_eq!(f.journal.snapshot().unwrap().leases[0].expires_at, 1020);
    assert!(matches!(
        f.gate.abort(f.invocation.key, Some(&token)),
        Err(Error::Conflict)
    ));
    assert_eq!(
        f.gate.completion(f.invocation.key).unwrap().output,
        c.output
    );
}

// An installed stage with a lost reply retains holds and requires an explicit original abort before admission can reopen.
#[test]
fn unknown_staging_and_invalid_accepted_factors() {
    let f = fixture();
    let mut bad = f.invocation.clone();
    bad.factors = Value::Object(vec![]);
    assert!(matches!(
        stage(&f.gate, &bad, &f.input, time(1), None, B),
        Err(Error::Invalid)
    ));
    assert!(f.journal.status(f.invocation.key).unwrap().is_none());
    f.faults.writes.lock().unwrap().push_back(Write::LoseReply);
    assert!(matches!(
        stage(&f.gate, &f.invocation, &f.input, time(1), None, B),
        Err(Error::Unknown)
    ));
    assert!(!f.gate.is_ready().unwrap());
    f.journal.reload().unwrap();
    assert!(matches!(
        f.gate.status(f.invocation.key),
        Ok(Some(Stage::Staged))
    ));
    f.gate.abort(f.invocation.key, None).unwrap();
    f.gate
        .recover(&Terminal(&f.journal), time(2), None)
        .unwrap();
    assert_eq!(f.journal.snapshot().unwrap().leases[0].expires_at, 1010);
    assert!(
        stage(
            &f.gate,
            &attempt(&f.invocation, 3),
            &f.input,
            time(2),
            None,
            B
        )
        .is_ok()
    );
}

// Seed an independently accepted remove proposal solely to exercise the existing participant's conflict boundary.
fn removal_proposal(
    f: &crate::modules::storage::test_support::mutations::Fixture,
    stream: u32,
    expected: u64,
) -> (services::contract::OperationKey, Completion) {
    let key = services::contract::OperationKey {
        stream,
        ..f.invocation.key
    };
    let effect = Effect::Remove {
        id: [8; 32],
        expected_expires_at: expected,
    }
    .encode(image_limits())
    .unwrap();
    let completion = Completion {
        output: Value::Bool(true),
        price: 2,
        result: Blob::new(vec![1], 1).unwrap(),
        effect: Blob::new(effect, 4096).unwrap(),
    };
    f.gate
        .admit(|journal, quota| {
            quota.reserve(
                key,
                Usage {
                    operations: 1,
                    result_bytes: 64,
                    ..Usage::default()
                },
            )?;
            journal.stage(key, [stream as u8; 32], completion.clone())
        })
        .unwrap();
    (key, completion)
}
// Renewal and removal serialize at preparation; the losing stale effect cannot overwrite the winner after commitment.
#[test]
fn renewal_removal_orderings() {
    for renew_first in [true, false] {
        let f = fixture();
        let renewal = stage(&f.gate, &f.invocation, &f.input, time(1), None, B).unwrap();
        let (remove_key, remove) = removal_proposal(&f, 3, 1010);
        let (first, first_effect, second, second_effect) = if renew_first {
            (
                f.invocation.key,
                &renewal.effect,
                remove_key,
                &remove.effect,
            )
        } else {
            (
                remove_key,
                &remove.effect,
                f.invocation.key,
                &renewal.effect,
            )
        };
        let token = f.gate.prepare(first, first_effect).unwrap();
        assert!(matches!(
            f.gate.prepare(second, second_effect),
            Err(Error::Conflict)
        ));
        f.gate.commit(first, &token).unwrap();
        assert!(matches!(
            f.gate.prepare(second, second_effect),
            Err(Error::Conflict)
        ));
        f.gate.abort(second, None).unwrap();
        if renew_first {
            assert_eq!(f.journal.snapshot().unwrap().leases[0].expires_at, 1020);
            let (key, remove) = removal_proposal(&f, 4, 1020);
            let token = f.gate.prepare(key, &remove.effect).unwrap();
            f.gate.commit(key, &token).unwrap();
        } else {
            assert!(matches!(
                stage(
                    &f.gate,
                    &attempt(&f.invocation, 4),
                    &f.input,
                    time(1),
                    None,
                    B
                ),
                Err(Error::Unavailable)
            ));
        }
        assert_eq!(
            f.journal.snapshot().unwrap().leases[0].state,
            super::super::contract::State::Removed
        );
    }
}
