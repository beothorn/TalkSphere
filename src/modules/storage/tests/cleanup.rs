//! Native bounded cleanup preserves immutable pins, guarded mutations and original result/identity history through failure and restart.
use super::*;
use crate::modules::{
    services::{
        contract::OperationKey,
        handler::{Blob, Completion},
        value::Value,
    },
    storage::{
        contract::State,
        effect::Effect,
        image::Stage,
        journal::Journal,
        quota::Usage,
        removal, renewal_commit,
        test_support::{
            Write,
            contracts::remove as remove_fixture,
            mutations::{B, CryptoPort, Fixture, Terminal, fixture, image_limits, limits, time},
        },
    },
};
use std::sync::Arc;
// Probe byte admission without retaining an unrelated hold; record/result and lease ID capacities remain separately bounded.
fn can_reserve_all_bytes(f: &Fixture) -> Result<(), Error> {
    let key = OperationKey {
        stream: 7,
        ..f.invocation.key
    };
    f.gate.admit(|_, quota| {
        quota.probe(
            key,
            Usage {
                bytes: 64,
                operations: 1,
                ..Usage::default()
            },
        )
    })
}
// Persist the actual accepted removal before attempting physical cleanup.
fn remove(f: &Fixture) {
    let (inv, input, _, _) = remove_fixture();
    let completion = removal::stage(&f.gate, &inv, &input, time(1), None, B).unwrap();
    let token = f.gate.prepare(inv.key, &completion.effect).unwrap();
    f.gate.commit(inv.key, &token).unwrap();
}
// Expiry becomes terminal at the exact deadline; byte capacity is released while original results and IDs survive owner reopen.
#[test]
fn expiry_reclamation_and_restarted_accounting() {
    let f = fixture();
    assert!(can_reserve_all_bytes(&f).is_err());
    assert_eq!(f.gate.collect(time(9), None, 1, None).unwrap().expired, 0);
    let result = f.gate.collect(time(10), None, 1, None).unwrap();
    assert_eq!(
        result,
        Cleanup {
            expired: 1,
            examined: 1,
            reclaimed: 1,
            pinned: 0,
            next: None
        }
    );
    assert_eq!(
        f.journal.snapshot().unwrap().leases[0].state,
        State::Expired
    );
    assert!(f.bytes.reopen([8; 32], 3).is_err());
    assert!(can_reserve_all_bytes(&f).is_ok());
    let mut corrected = time(11);
    corrected.unix_seconds = 0;
    f.gate.collect(corrected, None, 1, None).unwrap();
    assert_eq!(
        f.journal.snapshot().unwrap().leases[0].state,
        State::Expired
    );
    let journal = Arc::new(
        Journal::open(
            f.invocation.provider,
            f.faults.clone(),
            Arc::new(CryptoPort),
            image_limits(),
        )
        .unwrap(),
    );
    let gate = Gate::new(journal.clone(), f.bytes.clone(), limits()).unwrap();
    gate.recover(&Terminal(&journal), time(12), None).unwrap();
    gate.admit(|_, quota| {
        quota.probe(
            OperationKey {
                stream: 7,
                ..f.invocation.key
            },
            Usage {
                bytes: 64,
                operations: 1,
                ..Usage::default()
            },
        )
    })
    .unwrap();
    let image = journal.snapshot().unwrap();
    assert_eq!(image.leases.len(), 1);
    assert_eq!(image.operations.len(), 1);
    assert!(matches!(image.operations[0].status, Stage::Committed(_)));
    assert_eq!(
        journal.completion(image.operations[0].key).unwrap().price,
        17
    );
}
// Separately reopened handles and range clones share one pin lifetime; removal never releases their byte capacity early.
#[test]
fn range_and_handle_pins_defer_physical_reclamation() {
    let f = fixture();
    let first = f.bytes.reopen([8; 32], 3).unwrap();
    let second = f.bytes.reopen([8; 32], 3).unwrap();
    let range = first.range(1, 2).unwrap();
    let cloned = range.clone();
    drop(first);
    drop(range);
    remove(&f);
    let result = f.gate.collect(time(2), None, 1, None).unwrap();
    assert_eq!(result.pinned, 1);
    assert_eq!(result.reclaimed, 0);
    assert!(can_reserve_all_bytes(&f).is_err());
    drop(second);
    assert_eq!(cloned.read(0, 2, 2).unwrap(), b"bc");
    assert_eq!(f.gate.collect(time(3), None, 1, None).unwrap().pinned, 1);
    drop(cloned);
    assert_eq!(f.gate.collect(time(4), None, 1, None).unwrap().reclaimed, 1);
    assert!(can_reserve_all_bytes(&f).is_ok());
    assert_eq!(
        f.journal.snapshot().unwrap().leases[0].state,
        State::Removed
    );
    assert_eq!(f.gate.completion(remove_fixture().0.key).unwrap().price, 2);
}
// A staged/prepared extension protects its guard from cleanup until the coordinator applies its original decision.
#[test]
fn pending_renewal_protects_original_guard() {
    let f = fixture();
    let result = renewal_commit::stage(&f.gate, &f.invocation, &f.input, time(9), None, B).unwrap();
    let token = f.gate.prepare(f.invocation.key, &result.effect).unwrap();
    let cleanup = f.gate.collect(time(10), None, 1, None).unwrap();
    assert_eq!(cleanup.expired, 0);
    assert_eq!(cleanup.examined, 0);
    assert_eq!(f.journal.snapshot().unwrap().leases[0].expires_at, 1010);
    f.gate.commit(f.invocation.key, &token).unwrap();
    assert_eq!(f.gate.collect(time(11), None, 1, None).unwrap().expired, 0);
    assert_eq!(f.journal.snapshot().unwrap().leases[0].expires_at, 1020);
    assert_eq!(
        f.bytes.reopen([8; 32], 3).unwrap().read(0, 3, 3).unwrap(),
        b"abc"
    );
}
// Seed a second trusted test lease with actual bytes to exercise finite batch cursors; this is not paid store execution.
fn second_lease(f: &Fixture) {
    let mut lease = f.journal.snapshot().unwrap().leases[0].clone();
    lease.id = [9; 32];
    let key = OperationKey {
        stream: 4,
        ..f.invocation.key
    };
    f.gate
        .admit(|journal, quota| {
            quota.reserve(
                key,
                Usage {
                    bytes: 3,
                    leases: 1,
                    operations: 1,
                    result_bytes: 16,
                },
            )?;
            let mut upload = f.bytes.begin(lease.id, 3, 3)?;
            upload.append(0, b"abc")?;
            drop(upload.finish(lease.content_hash)?);
            let effect = Blob::new(Effect::Create(lease).encode(image_limits())?, 65536)?;
            journal.stage(
                key,
                [4; 32],
                Completion {
                    output: Value::Uint64(17),
                    result: Blob::new(vec![1], 1)?,
                    effect,
                    price: 17,
                },
            )
        })
        .unwrap();
    let completion = f.gate.completion(key).unwrap();
    let token = f.gate.prepare(key, &completion.effect).unwrap();
    f.gate.commit(key, &token).unwrap();
}
// Bounded batches expose a cursor so a pinned first object cannot starve a later reclaimable terminal object.
#[test]
fn finite_batches_and_pinned_cursor_progress() {
    let f = fixture();
    second_lease(&f);
    let pin = f.bytes.reopen([8; 32], 3).unwrap();
    let first = f.gate.collect(time(10), None, 1, None).unwrap();
    assert_eq!(first.expired, 2);
    assert_eq!(first.pinned, 1);
    assert_eq!(first.next, Some([8; 32]));
    let second = f.gate.collect(time(10), None, 1, first.next).unwrap();
    assert_eq!(second.reclaimed, 1);
    assert_eq!(second.next, None);
    drop(pin);
    assert_eq!(
        f.gate.collect(time(10), None, 1, None).unwrap().reclaimed,
        1
    );
    assert_eq!(f.journal.snapshot().unwrap().leases.len(), 2);
}
// Failed deletion leaves the terminal lease and charged byte capacity intact until a confirmed subsequent cleanup.
#[test]
fn confirmed_deletion_failure_keeps_capacity_and_terminal_state() {
    let f = fixture();
    remove(&f);
    f.bytes.fail_reclaim(1);
    assert_eq!(f.gate.collect(time(2), None, 1, None), Err(Error::Storage));
    assert!(f.gate.is_ready().unwrap());
    assert_eq!(
        f.journal.snapshot().unwrap().leases[0].state,
        State::Removed
    );
    assert!(can_reserve_all_bytes(&f).is_err());
    f.bytes.fail_reclaim(0);
    assert_eq!(f.gate.collect(time(2), None, 1, None).unwrap().reclaimed, 1);
    assert!(can_reserve_all_bytes(&f).is_ok());
}
// A lost post-unlink acknowledgement closes admission; recovery observes actual durable absence without resurrecting a lease.
#[test]
fn interrupted_unlink_recovers_without_erasing_history() {
    let f = fixture();
    remove(&f);
    f.bytes.fail_reclaim(2);
    assert_eq!(f.gate.collect(time(2), None, 1, None), Err(Error::Unknown));
    assert!(!f.gate.is_ready().unwrap());
    assert_eq!(
        f.gate.collect(time(2), None, 1, None),
        Err(Error::Unavailable)
    );
    f.bytes.fail_reclaim(0);
    f.gate
        .recover(&Terminal(&f.journal), time(3), None)
        .unwrap();
    assert_eq!(
        f.journal.snapshot().unwrap().leases[0].state,
        State::Removed
    );
    assert!(can_reserve_all_bytes(&f).is_ok());
    assert_eq!(f.gate.collect(time(3), None, 1, None).unwrap().reclaimed, 1);
    assert_eq!(f.gate.completion(remove_fixture().0.key).unwrap().price, 2);
}
// Failed expiry persistence cannot publish terminal state, while an installed-but-unacknowledged expiry requires reload.
#[test]
fn expiry_metadata_failures_and_clock_bounds() {
    let f = fixture();
    assert_eq!(f.gate.collect(time(10), None, 0, None), Err(Error::Invalid));
    f.faults.writes.lock().unwrap().push_back(Write::Fail);
    assert_eq!(
        f.gate.collect(time(10), None, 1, None),
        Err(Error::Unavailable)
    );
    assert_eq!(f.journal.snapshot().unwrap().leases[0].state, State::Active);
    f.faults.writes.lock().unwrap().push_back(Write::LoseReply);
    assert_eq!(f.gate.collect(time(10), None, 1, None), Err(Error::Unknown));
    assert!(!f.gate.is_ready().unwrap());
    f.gate
        .recover(&Terminal(&f.journal), time(11), None)
        .unwrap();
    assert_eq!(
        f.journal.snapshot().unwrap().leases[0].state,
        State::Expired
    );
    assert_eq!(
        f.gate.collect(time(11), None, 1, None).unwrap().reclaimed,
        1
    );
    assert_eq!(f.gate.collect(time(10), None, 1, None), Err(Error::Invalid));
}

// Unexpected external unlink cannot hide descriptor-retained bytes from recovered capacity accounting.
#[test]
fn descriptor_only_bytes_stay_charged_after_recovery() {
    let f = fixture();
    let pin = f.bytes.reopen([8; 32], 3).unwrap();
    remove(&f);
    std::fs::remove_file(f.path.0.join(format!("object-{}", "08".repeat(32)))).unwrap();
    f.gate
        .recover(&Terminal(&f.journal), time(2), None)
        .unwrap();
    assert!(can_reserve_all_bytes(&f).is_err());
    assert_eq!(pin.read(0, 3, 3).unwrap(), b"abc");
    assert_eq!(f.gate.collect(time(2), None, 1, None).unwrap().pinned, 1);
    drop(pin);
    assert_eq!(f.gate.collect(time(2), None, 1, None).unwrap().reclaimed, 1);
    assert!(can_reserve_all_bytes(&f).is_ok());
}
