//! Pinned complete Storage contract fixtures shared by independent provider/requester tests.
use crate::modules::{
    board::{config::Limits, definition::Definition, offering::Board},
    identity::api::IdentityId,
    services::{
        contract::OperationKey,
        handler::{Blob, Invocation},
        pricing::Price,
        value::Value,
    },
};
pub(crate) const HASH: [u8; 32] = [
    0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22, 0x23,
    0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00, 0x15, 0xad,
];
// Build typed scenarios directly from pinned complete contracts, never from the verifier's own output.
pub(crate) fn store(wanted: bool) -> (Invocation, Value, Value) {
    let offering: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/offering.json")).unwrap();
    let board = if wanted {
        serde_json::json!({"version":1,"providedServices":[],"wantedServices":[offering]})
    } else {
        serde_json::json!({"version":1,"providedServices":[offering],"wantedServices":[]})
    };
    let offering = Board::parse(&serde_json::to_vec(&board).unwrap(), Limits::default())
        .unwrap()
        .offerings
        .remove("storage-standard")
        .unwrap();
    let definition = Definition::parse(
        include_bytes!("../fixtures/definition.json"),
        &offering,
        Limits::default(),
    )
    .unwrap();
    let requester = IdentityId([2; 32]);
    let provider = IdentityId([1; 32]);
    let accepted = Blob::new(b"original accepted terms".to_vec(), 64).unwrap();
    let invocation = Invocation {
        key: OperationKey {
            requester,
            connection: [3; 32],
            stream: 1,
        },
        requester,
        provider,
        accepted: accepted.clone(),
        maximum: 17,
        publisher: if wanted { requester } else { provider },
        issuer: offering.issuer,
        offering,
        definition,
        quote: accepted,
        factors: Value::Object(vec![]),
        pricing: Price::Fixed(17),
    };
    let input = object(vec![
        ("operation", Value::String("store".into())),
        ("sizeBytes", Value::Uint64(3)),
        ("retentionSeconds", Value::Uint64(10)),
        ("contentHash", Value::Bytes(HASH.to_vec())),
        ("data", Value::Bytes(b"abc".to_vec())),
    ]);
    let output = object(vec![
        ("operation", Value::String("store".into())),
        ("leaseId", Value::Bytes(vec![8; 32])),
        ("sizeBytes", Value::Uint64(3)),
        ("contentHash", Value::Bytes(HASH.to_vec())),
        ("acceptedAt", Value::Uint64(1000)),
        ("expiresAt", Value::Uint64(1010)),
        ("price", Value::Uint64(17)),
    ]);
    (invocation, input, output)
}
// Retain exact logical kinds and field order only for scenario construction.
fn object(fields: Vec<(&str, Value)>) -> Value {
    Value::Object(fields.into_iter().map(|(n, v)| (n.into(), v)).collect())
}

// Build independent renewal input/output and trusted original lease data; this bypasses paid lease creation only in tests.
pub(crate) fn renew() -> (Invocation, Value, Value, super::super::contract::Lease) {
    use super::super::contract::{Lease, State};
    let (mut invocation, _, _) = store(false);
    let lease = Lease {
        id: [8; 32],
        requester: invocation.requester,
        provider: invocation.provider,
        accepted_terms: invocation.accepted.clone(),
        size_bytes: 3,
        content_hash: HASH,
        accepted_at: 1000,
        expires_at: 1010,
        store_price: 17,
        state: State::Active,
    };
    invocation.key.stream = 2;
    invocation.pricing = Price::Fixed(15);
    invocation.maximum = 15;
    invocation.factors = object(vec![("sizeBytes", Value::Uint64(3))]);
    let input = object(vec![
        ("operation", Value::String("renew".into())),
        ("leaseId", Value::Bytes(lease.id.to_vec())),
        ("expectedExpiresAt", Value::Uint64(1010)),
        ("additionalSeconds", Value::Uint64(10)),
    ]);
    let output = object(vec![
        ("operation", Value::String("renew".into())),
        ("leaseId", Value::Bytes(lease.id.to_vec())),
        ("previousExpiresAt", Value::Uint64(1010)),
        ("expiresAt", Value::Uint64(1020)),
        ("price", Value::Uint64(15)),
    ]);
    (invocation, input, output, lease)
}
