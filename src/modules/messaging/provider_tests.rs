//! Provider tests for assessing and committing stores (MS-02, MS-03).
use super::*;
use crate::modules::messaging::{
    operation::{
        CheckInput, Cursor, DiscoverInput, RecoverInput, RemoveInput, RenewInput, StatusInput,
    },
    receipt,
};
use crate::modules::{
    board::{config::Limits as BoardLimits, offering::Board},
    messaging::{
        ids::MessageId,
        journal::Journal,
        ports::Signer,
        state::State,
        store::{Limits, Store},
        test_support::{FixedClock, RecordingIssuer, SequenceEntropy, TestIdentity},
    },
};
use serde_json::Value;

pub(crate) struct Harness {
    pub provider: Provider,
    pub clock: Arc<FixedClock>,
    pub store: Arc<Store>,
    pub staging: Staging,
    pub sender: TestIdentity,
    pub recipient: TestIdentity,
    pub provider_identity: Arc<TestIdentity>,
}

// The acceptance-harness terms issued by the provider test identity (seed 2).
pub(crate) fn terms() -> Terms {
    let mut o: Value = serde_json::from_str(include_str!("fixtures/offering.json")).unwrap();
    let id = TestIdentity::new(2).identity();
    o["creditIssuer"] = crate::modules::identity::text::format(id).into();
    let board = serde_json::json!({"version":1,"providedServices":[o],"wantedServices":[]});
    let offering = Board::parse(&serde_json::to_vec(&board).unwrap(), BoardLimits::default())
        .unwrap()
        .offerings
        .remove("messaging-standard")
        .unwrap();
    Terms::parse(&offering, id).unwrap()
}

// A ready provider at time 1000 over volatile state: sender A (seed 1), provider B (seed 2), recipient C (seed 3).
pub(crate) fn harness() -> Harness {
    let clock = Arc::new(FixedClock::at(1000));
    let store = Arc::new(Store::volatile(Limits {
        placements: 16,
        payload_bytes: 10_000,
    }));
    let provider_identity = Arc::new(TestIdentity::new(2));
    let participant = Arc::new(Participant::new(
        store.clone(),
        Journal::volatile(64),
        Arc::new(SequenceEntropy::new(1)),
        clock.clone(),
        provider_identity.clone(),
    ));
    participant.recover(&NoDecisions).unwrap();
    let staging = Staging::new(1000, 4);
    let provider = Provider::new(terms(), participant, clock.clone(), staging.clone());
    Harness {
        provider,
        clock,
        store,
        staging,
        sender: TestIdentity::new(1),
        recipient: TestIdentity::new(3),
        provider_identity,
    }
}
struct NoDecisions;
impl super::super::recovery::Decisions for NoDecisions {
    fn decision(&self, _: OperationKey) -> Result<super::super::recovery::Decision, Error> {
        Ok(super::super::recovery::Decision::Undecided)
    }
}
pub(crate) fn key(n: u8) -> OperationKey {
    OperationKey {
        requester: IdentityId([n; 32]),
        connection: [n; 32],
        stream: u32::from(n),
    }
}
// A signed message from A to C with the given payload and encoding.
pub(crate) fn message(h: &Harness, payload: &[u8], encoding: Encoding) -> SignedMessage {
    signing::sign_message(
        &h.sender,
        h.recipient.identity(),
        MessageId([1; 32]),
        1000,
        encoding,
        payload.to_vec(),
    )
    .unwrap()
}
pub(crate) fn input(h: &Harness, payload: &[u8], retention: u64) -> StoreInput {
    StoreInput {
        message: message(h, payload, Encoding::Opaque),
        retention_seconds: retention,
    }
}

// MS-02: a store of `abc` for 10 seconds at time 1000 returns acceptedAt 1000, expiresAt 1010 and price 17, creates one pending placement
// with the payload, and a second store of the same message is a separate placement, not a deduplicated retry.
#[test]
fn ms_02_a_store_creates_a_durable_pending_placement() {
    let h = harness();
    let out = h
        .provider
        .store(key(1), h.sender.identity(), input(&h, b"abc", 10))
        .unwrap();
    assert_eq!(
        (out.accepted_at, out.expires_at, out.price),
        (1000, 1010, 17)
    );
    assert_eq!(out.message_id, MessageId([1; 32]));
    let record = h.store.record(out.placement_id).unwrap();
    assert_eq!(
        (record.state, record.payments, record.metadata.recipient_id),
        (State::Pending, 17, h.recipient.identity())
    );
    assert_eq!(
        h.store.get(out.placement_id).unwrap().unwrap().payload,
        b"abc"
    );
    let again = h
        .provider
        .store(key(2), h.sender.identity(), input(&h, b"abc", 10))
        .unwrap();
    assert_ne!(again.placement_id, out.placement_id);
    assert_eq!(h.store.records().len(), 2);
    assert_eq!(h.staging.used(), (0, 0), "reservations are released");
}

// MS-03 price: a one-byte payload for one second at half factors costs 1 and not 2; the provider reports the exact assessed price.
#[test]
fn ms_03_prices_round_once() {
    let h = harness();
    let one = h
        .provider
        .assess_store(h.sender.identity(), input(&h, b"a", 1).header())
        .unwrap();
    assert_eq!(one.price, 1);
    let abc = h
        .provider
        .assess_store(h.sender.identity(), input(&h, b"abc", 10).header())
        .unwrap();
    assert_eq!(abc.price, 17);
}

// MS-03: every rejection before commitment leaves no placement, no journal entry, no reservation and charges nothing.
#[test]
fn ms_03_rejections_leave_nothing_behind() {
    let h = harness();
    let a = h.sender.identity();
    let good = input(&h, b"abc", 10);
    let mut cases: Vec<(&str, IdentityId, StoreInput)> = Vec::new();
    let mut other_key = good.clone();
    other_key.message.sender_public_key = TestIdentity::new(5).key();
    cases.push(("sender key does not derive to the sender ID", a, other_key));
    cases.push((
        "sender is not the authenticated caller",
        IdentityId([9; 32]),
        good.clone(),
    ));
    let mut bad_signature = good.clone();
    bad_signature.message.sender_signature.0[0] ^= 1;
    cases.push(("invalid signature", a, bad_signature));
    let mut wrong_hash = good.clone();
    wrong_hash.message.metadata.payload_hash.0[0] ^= 1;
    cases.push(("wrong hash", a, wrong_hash));
    let mut other_payload = good.clone();
    other_payload.message.payload = b"abd".to_vec();
    cases.push(("payload differs from the signed one", a, other_payload));
    let mut zero_size = good.clone();
    zero_size.message.metadata.payload_size = 0;
    cases.push(("zero size", a, zero_size));
    let mut encrypted_short = good.clone();
    encrypted_short.message.metadata.encoding = Encoding::IdentityEncryptedV1;
    cases.push(("encrypted claim below 105 bytes", a, encrypted_short));
    for retention in [0u64, 101, u64::MAX] {
        cases.push((
            "retention outside the offering",
            a,
            StoreInput {
                retention_seconds: retention,
                ..good.clone()
            },
        ));
    }
    let oversized = StoreInput {
        message: message(&h, &[7u8; 101], Encoding::Opaque),
        retention_seconds: 10,
    };
    cases.push(("payload above the offering maximum", a, oversized));
    for (name, caller, case) in cases {
        assert!(h.provider.store(key(1), caller, case).is_err(), "{name}");
        assert_eq!(h.store.records().len(), 0, "{name}");
        assert_eq!(
            h.provider.participant().status(key(1)),
            super::super::participant::Status::Absent,
            "{name}"
        );
        assert_eq!(h.staging.used(), (0, 0), "{name}");
    }
}

// MS-03: an expiry that would overflow uint64 is refused even when the offering allows long retention.
#[test]
fn ms_03_time_addition_overflow_is_refused() {
    let h = harness();
    h.clock.set(u64::MAX - 5);
    assert!(
        h.provider
            .store(key(1), h.sender.identity(), input(&h, b"abc", 10))
            .is_err()
    );
    assert_eq!(h.store.records().len(), 0);
}

// The payload that arrives must be the one assessed: a different message, key or signature is refused after the header passed, and the
// reservation is released when the assessment is dropped.
#[test]
fn the_payload_must_match_the_assessed_header() {
    let h = harness();
    let good = input(&h, b"abc", 10);
    let assessment = h
        .provider
        .assess_store(h.sender.identity(), good.header())
        .unwrap();
    assert_eq!(h.staging.used(), (3, 1));
    let mut swapped = good.message.clone();
    swapped.metadata.message_id = MessageId([9; 32]);
    assert_eq!(
        h.provider.prepare_store(key(1), &assessment, swapped).err(),
        Some(Error::Invalid)
    );
    let mut tampered = good.message.clone();
    tampered.payload = b"xyz".to_vec();
    assert_eq!(
        h.provider
            .prepare_store(key(1), &assessment, tampered)
            .err(),
        Some(Error::Invalid)
    );
    assert_eq!(h.store.records().len(), 0);
    drop(assessment);
    assert_eq!(h.staging.used(), (0, 0));
}

// Staging bounds concurrent uploads: assessments beyond the byte or count budget are refused before any body is received.
#[test]
fn staging_bounds_concurrent_stores() {
    let h = harness();
    let held: Vec<_> = (0..4)
        .map(|_| {
            h.provider
                .assess_store(h.sender.identity(), input(&h, b"abc", 10).header())
                .unwrap()
        })
        .collect();
    assert_eq!(
        h.provider
            .assess_store(h.sender.identity(), input(&h, b"abc", 10).header())
            .err(),
        Some(Error::Limit)
    );
    drop(held);
    assert!(
        h.provider
            .assess_store(h.sender.identity(), input(&h, b"abc", 10).header())
            .is_ok()
    );
    // A payload larger than the remaining byte budget is refused too.
    let big = Staging::new(2, 4);
    let tight = Provider::new(
        terms(),
        h.provider.participant().clone(),
        h.clock.clone(),
        big,
    );
    assert_eq!(
        tight
            .assess_store(h.sender.identity(), input(&h, b"abc", 10).header())
            .err(),
        Some(Error::Limit)
    );
}

// A full placement store is refused when preparing, with the staging reservation released afterwards.
#[test]
fn a_full_store_refuses_new_placements() {
    let h = harness();
    let tiny = Arc::new(Store::volatile(Limits {
        placements: 1,
        payload_bytes: 10_000,
    }));
    let participant = Arc::new(Participant::new(
        tiny,
        Journal::volatile(8),
        Arc::new(SequenceEntropy::new(1)),
        h.clock.clone(),
        h.provider_identity.clone(),
    ));
    participant.recover(&NoDecisions).unwrap();
    let provider = Provider::new(terms(), participant, h.clock.clone(), h.staging.clone());
    provider
        .store(key(1), h.sender.identity(), input(&h, b"abc", 10))
        .unwrap();
    assert_eq!(
        provider
            .store(key(2), h.sender.identity(), input(&h, b"abc", 10))
            .err(),
        Some(Error::Limit)
    );
    assert_eq!(h.staging.used(), (0, 0));
}

// Stores `count` messages from A to C one second apart and returns their placement IDs in acceptance order.
fn store_many(h: &Harness, count: u8) -> Vec<PlacementId> {
    (0..count)
        .map(|n| {
            h.clock.set(1000 + u64::from(n));
            let message = signing::sign_message(
                &h.sender,
                h.recipient.identity(),
                MessageId([n + 1; 32]),
                1000,
                Encoding::Opaque,
                b"abc".to_vec(),
            )
            .unwrap();
            h.provider
                .store(
                    key(n + 1),
                    h.sender.identity(),
                    StoreInput {
                        message,
                        retention_seconds: 50,
                    },
                )
                .unwrap()
                .placement_id
        })
        .collect()
}

// MS-05: discovery pages through the recipient's pending placements in order, with an exclusive cursor, `more` only when another
// entry follows, and every page (including an empty one) charged the discover price.
#[test]
fn ms_05_discovery_pages_with_an_exclusive_cursor() {
    let h = harness();
    let ids = store_many(&h, 3);
    let c = h.recipient.identity();
    let first = h
        .provider
        .discover(
            c,
            DiscoverInput {
                cursor: None,
                limit: 2,
            },
        )
        .unwrap();
    assert_eq!(
        first
            .messages
            .iter()
            .map(|e| e.placement_id)
            .collect::<Vec<_>>(),
        ids[..2]
    );
    assert!(first.more);
    assert_eq!(first.price, h.provider.terms().discover_price);
    let last = first.messages.last().unwrap();
    let cursor = Cursor {
        after_accepted_at: last.accepted_at,
        after_placement_id: last.placement_id,
    };
    let second = h
        .provider
        .discover(
            c,
            DiscoverInput {
                cursor: Some(cursor),
                limit: 2,
            },
        )
        .unwrap();
    assert_eq!(
        second
            .messages
            .iter()
            .map(|e| e.placement_id)
            .collect::<Vec<_>>(),
        ids[2..]
    );
    assert!(!second.more);
    assert_eq!(second.messages[0].sender_id, h.sender.identity());
    assert_eq!(
        (
            second.messages[0].payload_size,
            second.messages[0].expires_at
        ),
        (3, 1052)
    );
    let empty = Cursor {
        after_accepted_at: second.messages[0].accepted_at,
        after_placement_id: second.messages[0].placement_id,
    };
    let done = h
        .provider
        .discover(
            c,
            DiscoverInput {
                cursor: Some(empty),
                limit: 2,
            },
        )
        .unwrap();
    assert!(done.messages.is_empty() && !done.more);
    assert_eq!(
        done.price,
        h.provider.terms().discover_price,
        "an empty page is a charged success"
    );
}

// MS-05/MS-04: only the authenticated recipient sees placements; other identities see an empty page, and the limit is bounded.
#[test]
fn discovery_is_recipient_only_and_bounded() {
    let h = harness();
    store_many(&h, 1);
    let other = h
        .provider
        .discover(
            h.sender.identity(),
            DiscoverInput {
                cursor: None,
                limit: 10,
            },
        )
        .unwrap();
    assert!(other.messages.is_empty());
    let max = h.provider.terms().maximum_discover_results;
    for limit in [0, max + 1] {
        assert_eq!(
            h.provider
                .discover(
                    h.recipient.identity(),
                    DiscoverInput {
                        cursor: None,
                        limit
                    }
                )
                .err(),
            Some(Error::Invalid)
        );
    }
    assert_eq!(
        h.provider
            .discover(
                h.recipient.identity(),
                DiscoverInput {
                    cursor: None,
                    limit: max
                }
            )
            .unwrap()
            .messages
            .len(),
        1
    );
}

// Expired and non-pending placements are not listed: after the retention ends the recipient sees nothing.
#[test]
fn discovery_hides_expired_placements() {
    let h = harness();
    store_many(&h, 2);
    // The placements expire at 1050 and 1051; a placement is expired from its expiry instant.
    h.clock.set(1050);
    let page = h
        .provider
        .discover(
            h.recipient.identity(),
            DiscoverInput {
                cursor: None,
                limit: 10,
            },
        )
        .unwrap();
    assert_eq!(page.messages.len(), 1);
    h.clock.set(1051);
    let page = h
        .provider
        .discover(
            h.recipient.identity(),
            DiscoverInput {
                cursor: None,
                limit: 10,
            },
        )
        .unwrap();
    assert!(page.messages.is_empty());
}

// Builds a request from the recipient for `placement` of the stored `abc` message with the given recovery ID.
pub(crate) fn recovery_request(h: &Harness, placement: PlacementId, recovery: u8) -> RecoverInput {
    let message = message(h, b"abc", Encoding::Opaque);
    receipt::sign_request(
        &h.recipient,
        h.provider_identity.identity(),
        placement,
        message.metadata.message_id,
        message.metadata.payload_hash,
        super::super::ids::RecoveryId([recovery; 32]),
    )
    .unwrap()
}

// MS-07: recovery returns the original metadata and payload with a receipt that verifies under both keys, moves the placement to
// recovered so discovery excludes it, and the result has no price field of its own.
#[test]
fn ms_07_recovery_returns_the_message_and_a_verifiable_receipt() {
    let h = harness();
    let stored = h
        .provider
        .store(key(1), h.sender.identity(), input(&h, b"abc", 10))
        .unwrap();
    h.clock.set(1004);
    let request = recovery_request(&h, stored.placement_id, 7);
    let (out, price) = h
        .provider
        .recover(key(2), h.recipient.identity(), request)
        .unwrap();
    assert_eq!(out.message, message(&h, b"abc", Encoding::Opaque));
    assert_eq!(out.receipt.recovered_at, 1004);
    assert_eq!(price, 2, "ceil(3 bytes * 1/2) rounded once");
    let binding = receipt::Binding {
        provider_id: h.provider_identity.identity(),
        placement_id: stored.placement_id,
        message_id: out.message.metadata.message_id,
        sender_id: h.sender.identity(),
        recipient_id: h.recipient.identity(),
        payload_hash: out.message.metadata.payload_hash,
    };
    receipt::verify_receipt(&out.receipt, &binding, &h.provider_identity.public_key()).unwrap();
    let record = h.store.record(stored.placement_id).unwrap();
    assert_eq!(
        (record.state, record.receipt),
        (State::Recovered, Some(out.receipt))
    );
    let page = h
        .provider
        .discover(
            h.recipient.identity(),
            DiscoverInput {
                cursor: None,
                limit: 10,
            },
        )
        .unwrap();
    assert!(page.messages.is_empty());
}

// Recovery is concealed and uncharged when the caller is not the recipient, the key does not match, the message or hash differs,
// the signature is bad, the placement is unknown or expired, or it was already recovered with another recovery ID.
#[test]
fn recovery_refusals_change_nothing() {
    let h = harness();
    let stored = h
        .provider
        .store(key(1), h.sender.identity(), input(&h, b"abc", 10))
        .unwrap();
    let p = stored.placement_id;
    let c = h.recipient.identity();
    let good = recovery_request(&h, p, 7);
    let mut bad_signature = good;
    bad_signature.recipient_signature.0[0] ^= 1;
    let mut wrong_message = good;
    wrong_message.message_id = MessageId([9; 32]);
    let mut wrong_hash = good;
    wrong_hash.payload_hash.0[0] ^= 1;
    let mut foreign_key = good;
    foreign_key.recipient_public_key = TestIdentity::new(5).public_key();
    let unknown = RecoverInput {
        placement_id: PlacementId([8; 32]),
        ..good
    };
    let cases = [
        (
            "caller is not the recipient",
            h.sender.identity(),
            good,
            Error::NotAvailable,
        ),
        ("bad signature", c, bad_signature, Error::Invalid),
        ("wrong message id", c, wrong_message, Error::NotAvailable),
        ("wrong hash", c, wrong_hash, Error::NotAvailable),
        (
            "key of another identity",
            c,
            foreign_key,
            Error::NotAvailable,
        ),
        ("unknown placement", c, unknown, Error::NotAvailable),
    ];
    for (name, caller, request, expected) in cases {
        assert_eq!(
            h.provider.recover(key(2), caller, request).err(),
            Some(expected),
            "{name}"
        );
        assert_eq!(h.store.record(p).unwrap().state, State::Pending, "{name}");
    }
    h.clock.set(1010);
    assert_eq!(
        h.provider.recover(key(2), c, good).err(),
        Some(Error::NotAvailable),
        "expired"
    );
    h.clock.set(1004);
    h.provider.recover(key(3), c, good).unwrap();
    let again = recovery_request(&h, p, 8);
    assert_eq!(
        h.provider.recover(key(4), c, again).err(),
        Some(Error::NotAvailable),
        "already recovered by another recovery id"
    );
}

// MS-08: an exact retry by the recipient (same recovery ID, a new operation) returns the identical output and the original price with
// no new effect, even after the offering price changed and after a renewal of the recovered placement; a different recovery ID is refused.
#[test]
fn ms_08_an_exact_retry_returns_the_cached_output_and_original_price() {
    let h = harness();
    let stored = h
        .provider
        .store(key(1), h.sender.identity(), input(&h, b"abc", 10))
        .unwrap();
    let c = h.recipient.identity();
    h.clock.set(1004);
    let request = recovery_request(&h, stored.placement_id, 7);
    let (first, price) = h.provider.recover(key(2), c, request).unwrap();
    let journal_before = h.provider.participant().status(key(3));
    let (retry, retry_price) = h.provider.recover(key(3), c, request).unwrap();
    assert_eq!((retry, retry_price), (first.clone(), price));
    assert_eq!(
        h.provider.participant().status(key(3)),
        journal_before,
        "a retry prepares nothing new"
    );
    // A repriced offering does not change the answer to the retry.
    let mut repriced = terms();
    repriced.recovery_usage = pricing::Rate {
        numerator: 9,
        denominator: 1,
    };
    let provider = Provider::new(
        repriced,
        h.provider.participant().clone(),
        h.clock.clone(),
        h.staging.clone(),
    );
    assert_eq!(provider.recover(key(4), c, request).unwrap().1, price);
    // Renewing the recovered placement leaves the cached output untouched.
    super::super::lifecycle::extend(&h.store, stored.placement_id, 1005, 1010, 5, 2000, 1).unwrap();
    let (after, after_price) = h.provider.recover(key(5), c, request).unwrap();
    assert_eq!((after, after_price), (first, price));
    let other = recovery_request(&h, stored.placement_id, 8);
    assert_eq!(
        h.provider.recover(key(6), c, other).err(),
        Some(Error::NotAvailable)
    );
}

// MS-09 and the status part of MS-04: only the sender sees the status; it is pending before recovery and carries the exact recorded
// receipt after it, never the payload, and the placement disappears from the sender's view when it expires or is unknown.
#[test]
fn ms_09_status_is_sender_only_and_shows_the_exact_receipt() {
    let h = harness();
    let stored = h
        .provider
        .store(key(1), h.sender.identity(), input(&h, b"abc", 10))
        .unwrap();
    let a = h.sender.identity();
    let ask = StatusInput {
        placement_id: stored.placement_id,
    };
    let pending = h.provider.status(a, ask).unwrap();
    assert_eq!(
        (pending.state, pending.receipt, pending.expires_at),
        (State::Pending, None, 1010)
    );
    assert_eq!(pending.price, h.provider.terms().status_price);
    assert_eq!(
        (pending.sender_id, pending.recipient_id),
        (a, h.recipient.identity())
    );
    for stranger in [h.recipient.identity(), IdentityId([9; 32])] {
        assert_eq!(
            h.provider.status(stranger, ask).err(),
            Some(Error::NotAvailable)
        );
    }
    h.clock.set(1004);
    let (recovered, _) = h
        .provider
        .recover(
            key(2),
            h.recipient.identity(),
            recovery_request(&h, stored.placement_id, 7),
        )
        .unwrap();
    let done = h.provider.status(a, ask).unwrap();
    assert_eq!(
        (done.state, done.receipt),
        (State::Recovered, Some(recovered.receipt))
    );
    assert_eq!(
        h.provider
            .status(
                a,
                StatusInput {
                    placement_id: PlacementId([8; 32])
                }
            )
            .err(),
        Some(Error::NotAvailable)
    );
    h.clock.set(1010);
    assert_eq!(
        h.provider.status(a, ask).err(),
        Some(Error::NotAvailable),
        "expired"
    );
}

// MS-10: a renewal needs the exact expiry the sender saw; it extends a pending placement without changing its state, charges the
// exact price, and a stale expectation, a foreign sender, a bad duration or the total limit are refused without any change.
#[test]
fn ms_10_renewal_is_guarded_by_the_expected_expiry() {
    let h = harness();
    let stored = h
        .provider
        .store(key(1), h.sender.identity(), input(&h, b"abc", 10))
        .unwrap();
    let a = h.sender.identity();
    let p = stored.placement_id;
    let ask = |expected: u64, seconds: u64| RenewInput {
        placement_id: p,
        expected_expires_at: expected,
        additional_seconds: seconds,
    };
    let out = h.provider.renew(key(2), a, ask(1010, 10)).unwrap();
    assert_eq!(
        (out.previous_expires_at, out.expires_at, out.price),
        (1010, 1020, 15)
    );
    let record = h.store.record(p).unwrap();
    assert_eq!(
        (record.state, record.expires_at, record.payments),
        (State::Pending, 1020, 17 + 15)
    );
    assert_eq!(
        h.provider.renew(key(3), a, ask(1010, 10)).err(),
        Some(Error::Conflict),
        "stale expectation"
    );
    assert_eq!(
        h.provider
            .renew(key(4), h.recipient.identity(), ask(1020, 10))
            .err(),
        Some(Error::NotAvailable)
    );
    for seconds in [0, 101] {
        assert_eq!(
            h.provider.renew(key(5), a, ask(1020, seconds)).err(),
            Some(Error::Invalid),
            "{seconds}"
        );
    }
    assert_eq!(h.store.record(p).unwrap().expires_at, 1020);
    // The total retention from acceptance (1000 + 200) is a hard ceiling.
    h.provider.renew(key(6), a, ask(1020, 100)).unwrap();
    assert_eq!(
        h.provider.renew(key(7), a, ask(1120, 100)).err(),
        Some(Error::Limit)
    );
    assert_eq!(h.store.record(p).unwrap().expires_at, 1120);
    assert_eq!(
        h.provider.participant().status(key(7)),
        super::super::participant::Status::Absent
    );
}

// MS-10: many concurrent renewals naming the same expected expiry produce exactly one winner; everyone else gets a conflict and pays nothing.
#[test]
fn ms_10_concurrent_renewals_have_one_winner() {
    let h = Arc::new(harness());
    let stored = h
        .provider
        .store(key(1), h.sender.identity(), input(&h, b"abc", 10))
        .unwrap();
    let results: Vec<_> = (0..8u8)
        .map(|n| {
            let h = h.clone();
            std::thread::spawn(move || {
                let ask = RenewInput {
                    placement_id: stored.placement_id,
                    expected_expires_at: 1010,
                    additional_seconds: 10,
                };
                h.provider.renew(key(10 + n), h.sender.identity(), ask)
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|t| t.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert!(
        results
            .iter()
            .filter_map(|r| r.as_ref().err())
            .all(|e| *e == Error::Conflict)
    );
    assert_eq!(
        h.store.record(stored.placement_id).unwrap().expires_at,
        1020
    );
}

// A release policy offering a fixed percentage of the payments, like the node's configured policy but without the minimum floor, so
// the tests can show that the module itself bounds the result.
struct Percent(u64);
impl super::super::ports::ReleasePolicy for Percent {
    fn release(&self, payments: u64, minimum: u64) -> u64 {
        (payments * self.0 / 100).max(minimum)
    }
}

// A provider over the harness's participant that offers `percent` of the payments as release credits.
pub(crate) fn releasing(h: &Harness, percent: u64) -> Provider {
    Provider::new(
        terms(),
        h.provider.participant().clone(),
        h.clock.clone(),
        h.staging.clone(),
    )
    .with_release_policy(Arc::new(Percent(percent)))
}

fn removal(p: PlacementId, expected: u64, minimum: u64) -> RemoveInput {
    RemoveInput {
        placement_id: p,
        removal_id: super::super::ids::RemovalId([4; 32]),
        expected_expires_at: expected,
        minimum_release_credits: minimum,
    }
}

// MS-11/MS-12 quote rules: the release amount is the policy's choice bounded by the minimum and by the cumulative store and renewal
// payments; later prices of other operations never enlarge the ceiling; nothing changes while quoting.
#[test]
fn quote_removal_bounds_release_credits_by_the_payments() {
    let h = harness();
    let stored = h
        .provider
        .store(key(1), h.sender.identity(), input(&h, b"abc", 10))
        .unwrap();
    let (a, p) = (h.sender.identity(), stored.placement_id);
    let half = releasing(&h, 50);
    let quote = half.quote_removal(a, &removal(p, 1010, 0)).unwrap();
    assert_eq!(
        (quote.release_credits, quote.price),
        (8, half.terms().remove_price),
        "50% of 17 rounds down"
    );
    // The minimum raises the offer up to the ceiling but never above the payments.
    assert_eq!(
        half.quote_removal(a, &removal(p, 1010, 12))
            .unwrap()
            .release_credits,
        12
    );
    assert_eq!(
        half.quote_removal(a, &removal(p, 1010, 17))
            .unwrap()
            .release_credits,
        17
    );
    assert_eq!(
        half.quote_removal(a, &removal(p, 1010, 18)).err(),
        Some(Error::Invalid)
    );
    // A policy that offers more than the payments is cut to them.
    assert_eq!(
        releasing(&h, 500)
            .quote_removal(a, &removal(p, 1010, 0))
            .unwrap()
            .release_credits,
        17
    );
    // Operations that charge other prices do not change the ceiling; renewals do.
    h.provider
        .discover(
            h.recipient.identity(),
            DiscoverInput {
                cursor: None,
                limit: 1,
            },
        )
        .unwrap();
    h.provider
        .status(a, StatusInput { placement_id: p })
        .unwrap();
    assert_eq!(
        releasing(&h, 100)
            .quote_removal(a, &removal(p, 1010, 0))
            .unwrap()
            .release_credits,
        17
    );
    h.provider
        .renew(
            key(2),
            a,
            RenewInput {
                placement_id: p,
                expected_expires_at: 1010,
                additional_seconds: 10,
            },
        )
        .unwrap();
    assert_eq!(
        releasing(&h, 100)
            .quote_removal(a, &removal(p, 1020, 0))
            .unwrap()
            .release_credits,
        17 + 15
    );
    // The default policy offers nothing, so a positive minimum is refused.
    assert_eq!(
        h.provider
            .quote_removal(a, &removal(p, 1020, 0))
            .unwrap()
            .release_credits,
        0
    );
    assert_eq!(
        h.provider.quote_removal(a, &removal(p, 1020, 1)).err(),
        Some(Error::Limit)
    );
    assert_eq!(h.store.record(p).unwrap().state, State::Pending);
}

// A removal can be quoted only by the sender of a pending, unexpired placement that still has the expiry the sender saw.
#[test]
fn quote_removal_refuses_unauthorized_or_stale_requests() {
    let h = harness();
    let stored = h
        .provider
        .store(key(1), h.sender.identity(), input(&h, b"abc", 10))
        .unwrap();
    let p = stored.placement_id;
    let half = releasing(&h, 50);
    assert_eq!(
        half.quote_removal(h.recipient.identity(), &removal(p, 1010, 0))
            .err(),
        Some(Error::NotAvailable)
    );
    assert_eq!(
        half.quote_removal(h.sender.identity(), &removal(PlacementId([9; 32]), 1010, 0))
            .err(),
        Some(Error::NotAvailable)
    );
    assert_eq!(
        half.quote_removal(h.sender.identity(), &removal(p, 1009, 0))
            .err(),
        Some(Error::Conflict)
    );
    h.clock.set(1004);
    h.provider
        .recover(key(2), h.recipient.identity(), recovery_request(&h, p, 7))
        .unwrap();
    assert_eq!(
        half.quote_removal(h.sender.identity(), &removal(p, 1010, 0))
            .err(),
        Some(Error::NotAvailable),
        "recovered"
    );
    let again = harness();
    let stored = again
        .provider
        .store(key(1), again.sender.identity(), input(&again, b"abc", 10))
        .unwrap();
    again.clock.set(1010);
    assert_eq!(
        releasing(&again, 50)
            .quote_removal(
                again.sender.identity(),
                &removal(stored.placement_id, 1010, 0)
            )
            .err(),
        Some(Error::NotAvailable),
        "expired"
    );
}

// A provider whose participant releases credits through a recording issuer that behaves like the Ledger (once per operation key).
fn removing(h: &Harness, percent: u64) -> (Provider, Arc<RecordingIssuer>) {
    let issuer = Arc::new(RecordingIssuer::default());
    let participant = Arc::new(
        Participant::new(
            h.store.clone(),
            Journal::volatile(64),
            Arc::new(SequenceEntropy::new(50)),
            h.clock.clone(),
            h.provider_identity.clone(),
        )
        .with_release_issuer(issuer.clone()),
    );
    participant.recover(&NoDecisions).unwrap();
    let provider = Provider::new(terms(), participant, h.clock.clone(), h.staging.clone())
        .with_release_policy(Arc::new(Percent(percent)));
    (provider, issuer)
}

// MS-11: a removal records the removal, releases the payload and issues the quoted credits to the sender exactly once, charging the
// removal price; the placement leaves discovery, status and recovery.
#[test]
fn ms_11_removal_issues_the_quoted_credits_once() {
    let h = harness();
    let (provider, issuer) = removing(&h, 50);
    let stored = provider
        .store(key(1), h.sender.identity(), input(&h, b"abc", 10))
        .unwrap();
    let p = stored.placement_id;
    let out = provider
        .remove(key(2), h.sender.identity(), removal(p, 1010, 4))
        .unwrap();
    assert_eq!(
        (out.removed, out.release_credits, out.price),
        (true, 8, provider.terms().remove_price)
    );
    assert_eq!(issuer.issued(), vec![(key(2), h.sender.identity(), 8)]);
    let record = h.store.record(p).unwrap();
    assert_eq!(record.state, State::Removed);
    assert!(h.store.get(p).is_err(), "the payload is released");
    assert!(
        provider
            .discover(
                h.recipient.identity(),
                DiscoverInput {
                    cursor: None,
                    limit: 5
                }
            )
            .unwrap()
            .messages
            .is_empty()
    );
    assert_eq!(
        provider
            .status(h.sender.identity(), StatusInput { placement_id: p })
            .err(),
        Some(Error::NotAvailable)
    );
    assert_eq!(
        provider
            .recover(key(3), h.recipient.identity(), recovery_request(&h, p, 7))
            .err(),
        Some(Error::NotAvailable)
    );
}

// MS-12 refusals: removal by a stranger, with a stale expiry, with a minimum above the payments or after recovery issues nothing and
// leaves the placement pending (or recovered); an exhausted issuer leaves the placement pending for a later retry.
#[test]
fn removal_refusals_issue_nothing() {
    let h = harness();
    let (provider, issuer) = removing(&h, 50);
    let stored = provider
        .store(key(1), h.sender.identity(), input(&h, b"abc", 10))
        .unwrap();
    let p = stored.placement_id;
    assert_eq!(
        provider
            .remove(key(2), h.recipient.identity(), removal(p, 1010, 0))
            .err(),
        Some(Error::NotAvailable)
    );
    assert_eq!(
        provider
            .remove(key(3), h.sender.identity(), removal(p, 1009, 0))
            .err(),
        Some(Error::Conflict)
    );
    assert_eq!(
        provider
            .remove(key(4), h.sender.identity(), removal(p, 1010, 18))
            .err(),
        Some(Error::Invalid)
    );
    issuer.fail_with(Some(Error::Insufficient));
    assert_eq!(
        provider
            .remove(key(5), h.sender.identity(), removal(p, 1010, 0))
            .err(),
        Some(Error::Insufficient)
    );
    assert_eq!(h.store.record(p).unwrap().state, State::Pending);
    assert!(issuer.issued().is_empty());
}

// MS-12 replay: an exact retry of a completed removal (same removal ID, a new operation, after the placement's expiry, and after a
// restart onto the same durable store) returns the identical output and issues nothing again; another removal ID or sender gets nothing.
#[test]
fn ms_12_an_exact_removal_retry_returns_the_recorded_result() {
    let h = harness();
    let (provider, issuer) = removing(&h, 50);
    let stored = provider
        .store(key(1), h.sender.identity(), input(&h, b"abc", 10))
        .unwrap();
    let p = stored.placement_id;
    let first = provider
        .remove(key(2), h.sender.identity(), removal(p, 1010, 4))
        .unwrap();
    let retry = provider
        .remove(key(3), h.sender.identity(), removal(p, 1010, 4))
        .unwrap();
    assert_eq!(retry, first);
    assert_eq!(
        provider.participant().status(key(3)),
        super::super::participant::Status::Absent,
        "nothing was prepared"
    );
    h.clock.set(5000);
    assert_eq!(
        provider
            .remove(key(4), h.sender.identity(), removal(p, 1010, 4))
            .unwrap(),
        first,
        "beyond the expiry"
    );
    // A restarted participant over the same store (and a fresh issuer that remembers nothing) answers from the record alone.
    let (restarted, fresh) = removing(&h, 50);
    assert_eq!(
        restarted
            .remove(key(5), h.sender.identity(), removal(p, 1010, 4))
            .unwrap(),
        first
    );
    assert!(fresh.issued().is_empty());
    assert_eq!(issuer.issued().len(), 1);
    let mut other_id = removal(p, 1010, 4);
    other_id.removal_id = super::super::ids::RemovalId([9; 32]);
    assert_eq!(
        restarted
            .remove(key(6), h.sender.identity(), other_id)
            .err(),
        Some(Error::NotAvailable)
    );
    assert_eq!(
        restarted
            .remove(key(7), h.recipient.identity(), removal(p, 1010, 4))
            .err(),
        Some(Error::NotAvailable)
    );
}

// MS-13 provider side: the sender gets exactly the payload byte at the offset of its pending placement; invalid offsets, other callers,
// unknown, expired, recovered and removed placements are refused, and a check changes nothing.
#[test]
fn ms_13_the_provider_serves_one_exact_byte_to_the_sender() {
    let h = harness();
    let stored = h
        .provider
        .store(key(1), h.sender.identity(), input(&h, b"abc", 10))
        .unwrap();
    let (a, p) = (h.sender.identity(), stored.placement_id);
    let ask = |offset: u64| CheckInput {
        placement_id: p,
        offset,
    };
    for (offset, byte) in [(0, b'a'), (1, b'b'), (2, b'c')] {
        let out = h.provider.check(a, ask(offset)).unwrap();
        assert_eq!((out.placement_id, out.offset, out.value), (p, offset, byte));
    }
    assert_eq!(h.provider.check(a, ask(3)).err(), Some(Error::Invalid));
    assert_eq!(
        h.provider.check(a, ask(u64::MAX)).err(),
        Some(Error::Invalid)
    );
    assert_eq!(
        h.provider.check(h.recipient.identity(), ask(0)).err(),
        Some(Error::NotAvailable)
    );
    assert_eq!(
        h.provider
            .check(
                a,
                CheckInput {
                    placement_id: PlacementId([8; 32]),
                    offset: 0
                }
            )
            .err(),
        Some(Error::NotAvailable)
    );
    assert_eq!(h.store.record(p).unwrap().state, State::Pending);
    h.clock.set(1004);
    h.provider
        .recover(key(2), h.recipient.identity(), recovery_request(&h, p, 7))
        .unwrap();
    assert_eq!(
        h.provider.check(a, ask(0)).err(),
        Some(Error::NotAvailable),
        "recovered"
    );
    let second = h.provider.store(key(3), a, input(&h, b"abc", 10)).unwrap();
    h.clock.set(1020);
    assert_eq!(
        h.provider
            .check(
                a,
                CheckInput {
                    placement_id: second.placement_id,
                    offset: 0
                }
            )
            .err(),
        Some(Error::NotAvailable),
        "expired"
    );
}

// Limits of one pending placement per sender or per recipient.
struct Tight(usize, usize);
impl super::super::ports::PendingLimits for Tight {
    fn per_sender(&self, _: IdentityId) -> usize {
        self.0
    }
    fn per_recipient(&self, _: IdentityId) -> usize {
        self.1
    }
}

// A provider over the harness's participant with the given pending limits.
fn limited(h: &Harness, sender: usize, recipient: usize) -> Provider {
    Provider::new(
        terms(),
        h.provider.participant().clone(),
        h.clock.clone(),
        h.staging.clone(),
    )
    .with_pending_limits(Arc::new(Tight(sender, recipient)))
}

// Sends one `abc` message from `from` to `to` through `provider`.
fn send(
    provider: &Provider,
    n: u8,
    from: &TestIdentity,
    to: IdentityId,
) -> Result<StoreOutput, Error> {
    let message = signing::sign_message(
        from,
        to,
        MessageId([n; 32]),
        1000,
        Encoding::Opaque,
        b"abc".to_vec(),
    )
    .unwrap();
    provider.store(
        key(n),
        from.identity(),
        StoreInput {
            message,
            retention_seconds: 10,
        },
    )
}

// MS-04 limits: a store is refused before any body or charge with Limit exactly when the sender or the recipient already holds the
// most pending placements allowed; other identities are unaffected, and finishing a placement frees its slot.
#[test]
fn pending_limits_bound_each_sender_and_recipient() {
    let h = harness();
    let provider = limited(&h, 2, 3);
    let other = TestIdentity::new(5);
    let c = h.recipient.identity();
    let first = send(&provider, 1, &h.sender, c).unwrap().placement_id;
    send(&provider, 2, &h.sender, c).unwrap();
    assert_eq!(
        send(&provider, 3, &h.sender, c).err(),
        Some(Error::Limit),
        "sender at its bound"
    );
    assert_eq!(h.staging.used(), (0, 0));
    assert_eq!(h.store.records().len(), 2);
    send(&provider, 4, &other, c).unwrap();
    assert_eq!(
        send(&provider, 5, &TestIdentity::new(6), c).err(),
        Some(Error::Limit),
        "recipient at its bound"
    );
    send(&provider, 6, &other, h.sender.identity()).unwrap();
    // A recovery ends a pending placement and frees a slot for its sender and its recipient.
    h.clock.set(1004);
    provider
        .recover(key(7), c, recovery_request(&h, first, 7))
        .unwrap();
    assert_eq!(send(&provider, 8, &h.sender, c).map(|_| ()), Ok(()));
}

// MS-04 indistinguishability: for every operation an unknown placement and another identity's placement produce exactly the same error,
// so a caller learns nothing about placements it does not own.
#[test]
fn unauthorized_and_unknown_placements_are_indistinguishable() {
    let h = harness();
    let (provider, _issuer) = removing(&h, 50);
    let stored = provider
        .store(key(1), h.sender.identity(), input(&h, b"abc", 10))
        .unwrap();
    let real = stored.placement_id;
    let unknown = PlacementId([8; 32]);
    let stranger = IdentityId([9; 32]);
    for who in [stranger, h.recipient.identity()] {
        let status = |p| provider.status(who, StatusInput { placement_id: p }).err();
        assert_eq!(status(real), status(unknown));
        let check = |p| {
            provider
                .check(
                    who,
                    CheckInput {
                        placement_id: p,
                        offset: 0,
                    },
                )
                .err()
        };
        assert_eq!(check(real), check(unknown));
        let renew = |p| {
            provider
                .renew(
                    key(20),
                    who,
                    RenewInput {
                        placement_id: p,
                        expected_expires_at: 1010,
                        additional_seconds: 5,
                    },
                )
                .err()
        };
        assert_eq!(renew(real), renew(unknown));
        let remove = |p| provider.remove(key(21), who, removal(p, 1010, 0)).err();
        assert_eq!(remove(real), remove(unknown));
    }
    let recover = |caller, p| {
        provider
            .recover(key(22), caller, recovery_request(&h, p, 7))
            .err()
    };
    assert_eq!(recover(stranger, real), recover(stranger, unknown));
    assert_eq!(
        recover(h.sender.identity(), real),
        recover(h.sender.identity(), unknown)
    );
}

// A recovery request from the recipient with its own recovery ID for the stored `abc` message.
fn recovery_with(h: &Harness, placement: PlacementId, recovery: u8) -> RecoverInput {
    recovery_request(h, placement, recovery)
}

// MS-16: racing recoveries (distinct recovery IDs) and removals of one pending placement produce exactly one transition. Every loser is
// refused as unavailable or conflicting, the final state is the winner's, release credits exist only if removal won, and discovery agrees.
#[test]
fn ms_16_recover_and_remove_race_has_one_winner() {
    for round in 0..20u8 {
        let h = Arc::new(harness());
        let (provider, issuer) = removing(&h, 50);
        let provider = Arc::new(provider);
        let id = provider
            .store(key(1), h.sender.identity(), input(&h, b"abc", 10))
            .unwrap()
            .placement_id;
        let workers: Vec<_> = (0..8u8)
            .map(|n| {
                let (h, provider) = (h.clone(), provider.clone());
                std::thread::spawn(move || {
                    if n % 2 == 0 {
                        provider
                            .recover(
                                key(10 + n),
                                h.recipient.identity(),
                                recovery_with(&h, id, n + 1),
                            )
                            .map(|_| "recovered")
                    } else {
                        let mut request = removal(id, 1010, 0);
                        request.removal_id = super::super::ids::RemovalId([n; 32]);
                        provider
                            .remove(key(10 + n), h.sender.identity(), request)
                            .map(|_| "removed")
                    }
                })
            })
            .collect();
        let results: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
        let winners: Vec<_> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
        assert_eq!(winners.len(), 1, "round {round}: {results:?}");
        assert!(
            results
                .iter()
                .filter_map(|r| r.as_ref().err())
                .all(|e| matches!(e, Error::NotAvailable | Error::Conflict))
        );
        let state = h.store.record(id).unwrap().state;
        match *winners[0] {
            "recovered" => assert_eq!((state, issuer.issued().len()), (State::Recovered, 0)),
            _ => assert_eq!((state, issuer.issued().len()), (State::Removed, 1)),
        }
        let listed = provider
            .discover(
                h.recipient.identity(),
                DiscoverInput {
                    cursor: None,
                    limit: 5,
                },
            )
            .unwrap()
            .messages
            .len();
        assert_eq!(listed, 0);
    }
}

// MS-16: a renewal and a removal naming the same expiry race to one winner; a renewal that wins makes the removal stale (it must be
// asked again with the new expiry) and a removal that wins makes the renewal unavailable.
#[test]
fn ms_16_renew_and_remove_race_has_one_winner() {
    for round in 0..20u8 {
        let h = Arc::new(harness());
        let (provider, _issuer) = removing(&h, 0);
        let provider = Arc::new(provider);
        let id = provider
            .store(key(1), h.sender.identity(), input(&h, b"abc", 10))
            .unwrap()
            .placement_id;
        let renew = {
            let (h, provider) = (h.clone(), provider.clone());
            std::thread::spawn(move || {
                provider
                    .renew(
                        key(2),
                        h.sender.identity(),
                        RenewInput {
                            placement_id: id,
                            expected_expires_at: 1010,
                            additional_seconds: 5,
                        },
                    )
                    .map(|_| ())
            })
        };
        let remove = {
            let (h, provider) = (h.clone(), provider.clone());
            std::thread::spawn(move || {
                provider
                    .remove(key(3), h.sender.identity(), removal(id, 1010, 0))
                    .map(|_| ())
            })
        };
        let (renewed, removed) = (renew.join().unwrap(), remove.join().unwrap());
        assert!(
            renewed.is_ok() != removed.is_ok(),
            "round {round}: {renewed:?} {removed:?}"
        );
        let record = h.store.record(id).unwrap();
        if renewed.is_ok() {
            assert_eq!(
                (record.state, record.expires_at, removed),
                (State::Pending, 1015, Err(Error::Conflict))
            );
        } else {
            // The renewal lost either to the finished removal (unavailable) or to its in-flight lock (conflict).
            assert_eq!(record.state, State::Removed);
            assert!(
                matches!(renewed, Err(Error::NotAvailable | Error::Conflict)),
                "{renewed:?}"
            );
        }
    }
}

// MS-16: every operation is available one second before the expiry and unavailable exactly at it, so near-expiry callers see either
// the whole operation or a clean refusal, never a half-applied state.
#[test]
fn ms_16_operations_end_exactly_at_the_expiry() {
    let h = harness();
    let (provider, _issuer) = removing(&h, 0);
    let a = h.sender.identity();
    let make = |n: u8| {
        provider
            .store(key(n), a, input(&h, b"abc", 10))
            .unwrap()
            .placement_id
    };
    let (r1, r2, r3, r4) = (make(1), make(2), make(3), make(4));
    h.clock.set(1009);
    assert!(provider.status(a, StatusInput { placement_id: r1 }).is_ok());
    assert!(
        provider
            .check(
                a,
                CheckInput {
                    placement_id: r1,
                    offset: 0
                }
            )
            .is_ok()
    );
    assert!(
        provider
            .renew(
                key(5),
                a,
                RenewInput {
                    placement_id: r2,
                    expected_expires_at: 1010,
                    additional_seconds: 1
                }
            )
            .is_ok()
    );
    assert!(provider.remove(key(6), a, removal(r3, 1010, 0)).is_ok());
    assert!(
        provider
            .recover(key(7), h.recipient.identity(), recovery_with(&h, r4, 7))
            .is_ok()
    );
    h.clock.set(1010);
    assert_eq!(
        provider.status(a, StatusInput { placement_id: r1 }).err(),
        Some(Error::NotAvailable)
    );
    assert_eq!(
        provider
            .check(
                a,
                CheckInput {
                    placement_id: r1,
                    offset: 0
                }
            )
            .err(),
        Some(Error::NotAvailable)
    );
    assert_eq!(
        provider
            .renew(
                key(8),
                a,
                RenewInput {
                    placement_id: r1,
                    expected_expires_at: 1010,
                    additional_seconds: 1
                }
            )
            .err(),
        Some(Error::NotAvailable)
    );
    assert_eq!(
        provider.remove(key(9), a, removal(r1, 1010, 0)).err(),
        Some(Error::NotAvailable)
    );
    assert_eq!(
        provider
            .recover(key(10), h.recipient.identity(), recovery_with(&h, r1, 8))
            .err(),
        Some(Error::NotAvailable)
    );
}

// MS-16: a sender polling status while the recipient recovers only ever sees a state that existed: pending without a receipt or
// recovered with the complete receipt, never a mixture.
#[test]
fn ms_16_status_observes_only_whole_states() {
    let h = Arc::new(harness());
    let id = h
        .provider
        .store(key(1), h.sender.identity(), input(&h, b"abc", 10))
        .unwrap()
        .placement_id;
    let reader = {
        let h = h.clone();
        std::thread::spawn(move || {
            let mut seen = Vec::new();
            for _ in 0..200 {
                let s = h
                    .provider
                    .status(h.sender.identity(), StatusInput { placement_id: id })
                    .unwrap();
                seen.push((s.state, s.receipt.is_some()));
            }
            seen
        })
    };
    h.provider
        .recover(key(2), h.recipient.identity(), recovery_with(&h, id, 7))
        .unwrap();
    let seen = reader.join().unwrap();
    assert!(
        seen.iter()
            .all(|s| matches!(s, (State::Pending, false) | (State::Recovered, true))),
        "{seen:?}"
    );
    let last = h
        .provider
        .status(h.sender.identity(), StatusInput { placement_id: id })
        .unwrap();
    assert_eq!(
        (last.state, last.receipt.is_some()),
        (State::Recovered, true)
    );
}
