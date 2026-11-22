//! Reclaim tests: finished placements free their resources and nothing still owed is lost.
use super::*;
use crate::modules::messaging::{
    ids::PlacementId,
    operation::{DiscoverInput, RemoveInput, StatusInput},
    ports::Signer,
    provider::tests::{Harness, harness, input, key, recovery_request},
};

// Stores one `abc` message for 10 seconds at the harness clock and returns its placement.
fn stored(h: &Harness, n: u8) -> PlacementId {
    h.provider
        .store(key(n), h.sender.identity(), input(h, b"abc", 10))
        .unwrap()
        .placement_id
}

// Pending placements and unexpired recovered placements are never reclaimed: the recipient can still discover and recover the first
// and the recovery retry and sender status still read the second, payload included.
#[test]
fn reclaim_keeps_everything_still_owed() {
    let h = harness();
    let pending = stored(&h, 1);
    let recovered = stored(&h, 2);
    h.clock.set(1004);
    let request = recovery_request(&h, recovered, 7);
    let (first, price) = h
        .provider
        .recover(key(3), h.recipient.identity(), request)
        .unwrap();
    let report = h.provider.participant().reclaim().unwrap();
    assert_eq!(report, Report::default());
    assert!(h.store.get(pending).unwrap().is_some() && h.store.get(recovered).unwrap().is_some());
    assert_eq!(
        h.provider
            .recover(key(4), h.recipient.identity(), request)
            .unwrap(),
        (first, price),
        "the retry still works"
    );
    assert!(
        h.provider
            .status(
                h.sender.identity(),
                StatusInput {
                    placement_id: recovered
                }
            )
            .is_ok()
    );
    assert_eq!(
        h.provider
            .discover(
                h.recipient.identity(),
                DiscoverInput {
                    cursor: None,
                    limit: 5
                }
            )
            .unwrap()
            .messages
            .len(),
        1
    );
}

// Expired placements (pending or recovered) lose their payload and record, freeing the slot; running again changes nothing.
#[test]
fn reclaim_frees_expired_placements_once() {
    let h = harness();
    let pending = stored(&h, 1);
    let recovered = stored(&h, 2);
    h.clock.set(1004);
    h.provider
        .recover(
            key(3),
            h.recipient.identity(),
            recovery_request(&h, recovered, 7),
        )
        .unwrap();
    h.clock.set(1010);
    let report = h.provider.participant().reclaim().unwrap();
    assert_eq!(
        report,
        Report {
            expired: 2,
            payloads_released: 2,
            records_dropped: 2,
            skipped_locked: 0
        }
    );
    assert!(h.store.record(pending).is_none() && h.store.record(recovered).is_none());
    assert_eq!(
        h.provider.participant().reclaim().unwrap(),
        Report::default()
    );
}

// A removed placement releases its payload but keeps its tombstone, so an exact removal retry is still answered long after expiry.
#[test]
fn reclaim_keeps_removal_tombstones() {
    use crate::modules::messaging::{ids::RemovalId, provider::tests::releasing};
    let h = harness();
    // A policy offering no credits keeps the settlement independent of any issuer.
    let provider = releasing(&h, 0);
    let id = provider
        .store(key(1), h.sender.identity(), input(&h, b"abc", 10))
        .unwrap()
        .placement_id;
    let removal = RemoveInput {
        placement_id: id,
        removal_id: RemovalId([4; 32]),
        expected_expires_at: 1010,
        minimum_release_credits: 0,
    };
    let first = provider
        .remove(key(2), h.sender.identity(), removal)
        .unwrap();
    h.clock.set(9000);
    let report = provider.participant().reclaim().unwrap();
    assert_eq!(report.records_dropped, 0);
    assert_eq!(
        provider
            .remove(key(3), h.sender.identity(), removal)
            .unwrap(),
        first
    );
    assert_eq!(h.store.record(id).unwrap().state, State::Removed);
}

// A placement held by a prepared operation is skipped until the coordinator decides, even when it has expired by the clock.
#[test]
fn reclaim_skips_placements_with_a_prepared_operation() {
    use crate::modules::messaging::{effect::Effect, operation::RecoverInput};
    let h = harness();
    let id = stored(&h, 1);
    let request: RecoverInput = recovery_request(&h, id, 7);
    let token = h
        .provider
        .participant()
        .prepare(
            key(2),
            &Effect::Recover {
                input: request,
                price: 2,
            },
        )
        .unwrap();
    h.clock.set(1010);
    let report = h.provider.participant().reclaim().unwrap();
    assert_eq!(
        report,
        Report {
            skipped_locked: 1,
            ..Report::default()
        }
    );
    assert!(h.store.record(id).is_some());
    h.provider
        .participant()
        .abort(key(2), Some(&token))
        .unwrap();
    assert_eq!(
        h.provider.participant().reclaim().unwrap().records_dropped,
        1
    );
}

// A failing record removal stops the pass without losing anything: the placement is still recorded and the next pass finishes it.
#[test]
fn reclaim_resumes_after_a_storage_failure() {
    let h = harness();
    let id = stored(&h, 1);
    h.clock.set(1010);
    h.store.fail_writes(true);
    assert!(h.provider.participant().reclaim().is_err());
    h.store.fail_writes(false);
    assert!(h.provider.participant().reclaim().is_ok());
    assert!(h.store.record(id).is_none());
}
