//! Provider tests for assessing and committing stores (MS-02, MS-03).
use super::*;
use crate::modules::{
    board::{config::Limits as BoardLimits, offering::Board},
    messaging::{
        ids::MessageId,
        journal::Journal,
        ports::Signer,
        state::State,
        store::{Limits, Store},
        test_support::{FixedClock, SequenceEntropy, TestIdentity},
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
fn input(h: &Harness, payload: &[u8], retention: u64) -> StoreInput {
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
