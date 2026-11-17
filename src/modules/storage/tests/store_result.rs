//! Requester-owned original data, accepted terms and committed-catalog publication checks.
use super::*;
use crate::modules::{
    board::config::Limits,
    identity::api::IdentityId,
    services::{
        contract::OperationKey,
        handler::{Blob, Completion},
        value::Value,
    },
    storage::requester::catalog::{self, Catalog, Committed},
};
use std::{
    collections::BTreeMap,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
const B: Budget = Budget {
    depth: 8,
    nodes: 128,
    bytes: 4096,
};
const POLICY: Policy = Policy {
    observed_unix_seconds: 1000,
    maximum_clock_difference_seconds: 2,
};
use crate::modules::storage::test_support::contracts::{HASH, store as fixture};
// Mutate one scenario field so each rejection has an independent observable cause.
fn set(value: &mut Value, name: &str, replacement: Value) {
    let Value::Object(f) = value else {
        panic!("fixture object")
    };
    f.iter_mut().find(|(n, _)| n == name).unwrap().1 = replacement;
}
// Both economic directions retain actual requester/provider roles and the exact original accepted binding.
#[test]
fn valid_store_metadata_and_wanted_roles() {
    for wanted in [false, true] {
        let (inv, input, output) = fixture(wanted);
        let lease = validate(&inv, &input, &output, B, POLICY).unwrap();
        assert_eq!(lease.id, [8; 32]);
        assert_eq!(lease.requester, inv.requester);
        assert_eq!(lease.provider, inv.provider);
        assert_eq!(lease.size_bytes, 3);
        assert_eq!(lease.content_hash, HASH);
        assert_eq!(lease.accepted_at, 1000);
        assert_eq!(lease.expires_at, 1010);
        assert_eq!(lease.store_price, 17);
        assert!(lease.accepted_terms == inv.accepted);
    }
}
// Missing, wrong-kind and inconsistent result fields cannot create successful lease metadata.
#[test]
fn malformed_or_inconsistent_results() {
    let (inv, input, output) = fixture(false);
    let Value::Object(fields) = &output else {
        unreachable!()
    };
    for index in 0..fields.len() {
        let mut bad = fields.clone();
        bad.remove(index);
        assert!(validate(&inv, &input, &Value::Object(bad), B, POLICY).is_err());
        let mut bad = output.clone();
        set(&mut bad, &fields[index].0, Value::Bool(false));
        assert!(validate(&inv, &input, &bad, B, POLICY).is_err());
    }
    for (name, value) in [
        ("sizeBytes", Value::Uint64(4)),
        ("contentHash", Value::Bytes(vec![0; 32])),
        ("leaseId", Value::Bytes(vec![8; 31])),
        ("price", Value::Uint64(18)),
        ("expiresAt", Value::Uint64(1011)),
        ("acceptedAt", Value::Uint64(990)),
    ] {
        let mut bad = output.clone();
        set(&mut bad, name, value);
        assert!(validate(&inv, &input, &bad, B, POLICY).is_err());
    }
    let mut bad = input.clone();
    set(&mut bad, "data", Value::Bytes(b"abd".to_vec()));
    assert!(matches!(
        validate(&inv, &bad, &output, B, POLICY),
        Err(Error::Invalid)
    ));
    assert!(matches!(
        validate(&inv, &input, &output, Budget { bytes: 1, ..B }, POLICY),
        Err(Error::Limit)
    ));
}
// Original invocation authority and price cannot be replaced by peer-result fields or changed local bindings.
#[test]
fn mismatched_original_context_and_integer_extrema() {
    let (inv, input, output) = fixture(false);
    for changed in 0..6 {
        let mut bad = inv.clone();
        match changed {
            0 => bad.publisher = IdentityId([9; 32]),
            1 => bad.key.requester = IdentityId([9; 32]),
            2 => bad.issuer = IdentityId([9; 32]),
            3 => bad.maximum = 18,
            4 => bad.pricing = Price::Fixed(18),
            _ => bad.factors = Value::Object(vec![("sizeBytes".into(), Value::Uint64(3))]),
        };
        assert!(validate(&bad, &input, &output, B, POLICY).is_err());
    }
    let mut high = output.clone();
    set(&mut high, "acceptedAt", Value::Uint64(u64::MAX - 10));
    set(&mut high, "expiresAt", Value::Uint64(u64::MAX));
    assert_eq!(
        validate(
            &inv,
            &input,
            &high,
            B,
            Policy {
                observed_unix_seconds: u64::MAX - 10,
                maximum_clock_difference_seconds: 0
            }
        )
        .unwrap()
        .expires_at,
        u64::MAX
    );
    set(&mut high, "acceptedAt", Value::Uint64(u64::MAX));
    assert!(matches!(
        validate(
            &inv,
            &input,
            &high,
            B,
            Policy {
                observed_unix_seconds: u64::MAX,
                maximum_clock_difference_seconds: 0
            }
        ),
        Err(Error::Limit)
    ));
    let mut equivalent = inv.clone();
    let mut d = equivalent.definition.document.value().clone();
    d["serviceVersion"] = serde_json::from_str("1.0").unwrap();
    equivalent.definition.document = crate::modules::board::document::Document::parse(
        &serde_json::to_vec(&d).unwrap(),
        Limits::default(),
    )
    .unwrap();
    assert!(validate(&equivalent, &input, &output, B, POLICY).is_ok());
}
struct Source {
    invocation: Invocation,
    completion: Completion,
    error: Option<Error>,
    identity: IdentityId,
    calls: AtomicUsize,
}
impl Committed for Source {
    // This is an explicit trusted observation substitute, not a proof of actual Services/peer completion.
    fn local_identity(&self) -> IdentityId {
        self.identity
    }
    // Inject unavailable/uncommitted observation independently from any provider output.
    fn completed(&self, _key: OperationKey) -> Result<(Invocation, Completion), Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(error) = self.error {
            return Err(error);
        }
        Ok((self.invocation.clone(), self.completion.clone()))
    }
}
#[derive(Default)]
struct Sink {
    rows: Mutex<BTreeMap<(IdentityId, [u8; 32]), Lease>>,
    calls: AtomicUsize,
    fail: AtomicBool,
}
impl Catalog for Sink {
    // Application-owned persistence accepts exact repeats and rejects conflicting reuse without touching service accounting.
    fn retain(&self, lease: &Lease) -> Result<(), Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail.load(Ordering::SeqCst) {
            return Err(Error::Storage);
        }
        let mut rows = self.rows.lock().unwrap();
        let key = (lease.provider, lease.id);
        if let Some(old) = rows.get(&key) {
            if old.content_hash != lease.content_hash
                || old.size_bytes != lease.size_bytes
                || old.expires_at != lease.expires_at
                || old.accepted_terms != lease.accepted_terms
            {
                return Err(Error::Conflict);
            }
            return Ok(());
        }
        rows.insert(key, lease.clone());
        Ok(())
    }
}
// Catalog retries observe the same local committed operation and never cause another remote store or charge.
#[test]
fn catalog_publication_requires_original_acknowledged_result() {
    let (inv, input, output) = fixture(false);
    let completion = Completion {
        output,
        result: Blob::new(vec![1], 1).unwrap(),
        effect: Blob::new(vec![0], 1).unwrap(),
        price: 17,
    };
    let mut source = Source {
        identity: inv.requester,
        invocation: inv,
        completion,
        error: Some(Error::Unknown),
        calls: AtomicUsize::new(0),
    };
    let sink = Sink::default();
    let key = source.invocation.key;
    assert!(matches!(
        catalog::remember(&source, &sink, key, &input, B, POLICY),
        Err(Error::Unknown)
    ));
    assert_eq!(sink.calls.load(Ordering::SeqCst), 0);
    source.error = None;
    source.identity = IdentityId([9; 32]);
    assert!(matches!(
        catalog::remember(&source, &sink, key, &input, B, POLICY),
        Err(Error::Denied)
    ));
    assert_eq!(sink.calls.load(Ordering::SeqCst), 0);
    source.identity = source.invocation.requester;
    assert!(matches!(
        catalog::remember(
            &source,
            &sink,
            OperationKey { stream: 2, ..key },
            &input,
            B,
            POLICY
        ),
        Err(Error::Denied)
    ));
    assert_eq!(sink.calls.load(Ordering::SeqCst), 0);
    source.completion.price = 18;
    assert!(matches!(
        catalog::remember(&source, &sink, key, &input, B, POLICY),
        Err(Error::Invalid)
    ));
    assert_eq!(sink.calls.load(Ordering::SeqCst), 0);
    source.completion.price = 17;
    sink.fail.store(true, Ordering::SeqCst);
    assert!(matches!(
        catalog::remember(&source, &sink, key, &input, B, POLICY),
        Err(Error::Storage)
    ));
    assert!(sink.rows.lock().unwrap().is_empty());
    sink.fail.store(false, Ordering::SeqCst);
    catalog::remember(&source, &sink, key, &input, B, POLICY).unwrap();
    catalog::remember(&source, &sink, key, &input, B, POLICY).unwrap();
    assert_eq!(sink.rows.lock().unwrap().len(), 1);
    assert_eq!(
        sink.rows.lock().unwrap()[&(source.invocation.provider, [8; 32])].content_hash,
        HASH
    );
}
