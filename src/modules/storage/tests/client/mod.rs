//! Installed requester tests use independent accepted fixtures and explicit original local expectations, not a production provider substitute.
mod checks;
mod quote;
mod result;
use super::*;
use crate::modules::{
    board::config::Limits as BoardLimits,
    services::{
        handler::{Peer, Requester},
        validation::Semantics,
        value::{Budget, Value},
    },
    storage::{
        contract::{State, Time},
        test_support::contracts::{
            remove as remove_fixture, renew as renew_fixture, store as store_fixture,
        },
    },
};
use std::{collections::BTreeMap, sync::Mutex};
const B: Budget = Budget {
    depth: 8,
    nodes: 128,
    bytes: 8192,
};
struct ClockPort(Mutex<Result<Time, Error>>);
impl Clock for ClockPort {
    // Return the independently chosen requester receipt observation, or preserve a simulated local clock failure.
    fn now(&self) -> Result<Time, Error> {
        *self.0.lock().unwrap()
    }
}
// Original attempt, provider, requester and opaque ID form the independent snapshot key.
type SnapshotKey = (OperationKey, IdentityId, IdentityId, [u8; 32]);
#[derive(Default)]
struct Local {
    leases: Mutex<BTreeMap<SnapshotKey, Lease>>,
    ranges: Mutex<BTreeMap<OperationKey, Blob>>,
    fail: Mutex<Option<Error>>,
    range_error: Mutex<Option<Error>>,
    range_calls: std::sync::atomic::AtomicUsize,
}
impl Leases for Local {
    // The fixture indexes all original provider/requester/attempt fields so identical opaque IDs at different providers remain distinct.
    fn lease(
        &self,
        key: OperationKey,
        provider: IdentityId,
        requester: IdentityId,
        id: [u8; 32],
    ) -> Result<Lease, Error> {
        if let Some(error) = *self.fail.lock().unwrap() {
            return Err(error);
        }
        self.leases
            .lock()
            .unwrap()
            .get(&(key, provider, requester, id))
            .cloned()
            .ok_or(Error::Denied)
    }
    // Original private bytes are returned only from local expectation storage for this original attempt.
    fn range(
        &self,
        key: OperationKey,
        _provider: IdentityId,
        _id: [u8; 32],
        _offset: u64,
        _length: u64,
    ) -> Result<Option<Blob>, Error> {
        self.range_calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if let Some(error) = *self.range_error.lock().unwrap() {
            return Err(error);
        }

        if let Some(error) = *self.fail.lock().unwrap() {
            return Err(error);
        }
        Ok(self.ranges.lock().unwrap().get(&key).cloned())
    }
}
struct Fixture {
    client: Client,
    local: Arc<Local>,
    clock: Arc<ClockPort>,
}
// Create a real installed requester over independently specified lease snapshots and a trusted fixed receipt clock.
fn fixture() -> Fixture {
    let (inv, _, _, lease) = renew_fixture();
    let local = Arc::new(Local::default());
    local.leases.lock().unwrap().insert(
        (inv.key, inv.provider, inv.requester, lease.id),
        lease.clone(),
    );
    let (remove, _, _, _) = remove_fixture();
    local.leases.lock().unwrap().insert(
        (remove.key, remove.provider, remove.requester, lease.id),
        lease,
    );
    let clock = Arc::new(ClockPort(Mutex::new(Ok(Time {
        unix_seconds: 1001,
        elapsed_seconds: 1,
        boot: [1; 16],
    }))));
    let catalogue = Catalogue::new(BoardLimits::default()).unwrap();
    let client = Client::new(
        inv.definition,
        &catalogue,
        local.clone(),
        clock.clone(),
        Policy {
            values: B,
            maximum_clock_difference_seconds: 2,
        },
    )
    .unwrap();
    Fixture {
        client,
        local,
        clock,
    }
}
// Preserve explicit application requester/provider identities independently of credit issuer and TCP initiator.
fn peer(inv: &crate::modules::services::handler::Invocation, provider_side: bool) -> Peer {
    Peer {
        local: if provider_side {
            inv.provider
        } else {
            inv.requester
        },
        remote: if provider_side {
            inv.requester
        } else {
            inv.provider
        },
        key: inv.key,
    }
}
// Replace one logical fixture field without depending on the installed requester's encoder or validation algorithm.
fn set(value: &mut Value, name: &str, replacement: Value) {
    let Value::Object(fields) = value else {
        panic!("fixture object")
    };
    fields.iter_mut().find(|(n, _)| n == name).unwrap().1 = replacement;
}
// Construct a full-object retrieval from independently retained abc metadata and exact current two-credit price.
fn retrieve() -> (
    crate::modules::services::handler::Invocation,
    Value,
    Value,
    Lease,
) {
    let (mut inv, _, _, lease) = renew_fixture();
    inv.pricing = crate::modules::services::pricing::Price::Fixed(2);
    inv.maximum = 2;
    inv.factors = Value::Object(vec![]);
    let input = Value::Object(vec![
        ("operation".into(), Value::String("retrieve".into())),
        ("leaseId".into(), Value::Bytes(lease.id.to_vec())),
        ("offset".into(), Value::Uint64(0)),
        ("length".into(), Value::Uint64(3)),
    ]);
    let output = Value::Object(vec![
        ("operation".into(), Value::String("retrieve".into())),
        ("leaseId".into(), Value::Bytes(lease.id.to_vec())),
        ("offset".into(), Value::Uint64(0)),
        ("data".into(), Value::Bytes(b"abc".to_vec())),
        (
            "contentHash".into(),
            Value::Bytes(lease.content_hash.to_vec()),
        ),
        ("price".into(), Value::Uint64(2)),
    ]);
    (inv, input, output, lease)
}
