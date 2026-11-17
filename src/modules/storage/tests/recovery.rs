//! Real durable owner reopen, coordinator substitution and fail-closed resource admission.
use super::*;
use crate::modules::{
    cryptography::primitives::hash::sha256,
    services::{
        handler::{Blob, Completion},
        value::{Budget, Value},
    },
    storage::{
        bytes::tests::{PathGuard, path},
        contract::{Crypto, Lease, State},
        effect::Effect,
        image::Limits as ImageLimits,
        quota::Usage,
        retention::Anchor,
    },
};
use std::sync::atomic::{AtomicUsize, Ordering};
struct CryptoPort;
impl Crypto for CryptoPort {
    // This explicitly test-only entropy substitute does not establish authenticated networking.
    fn lease_id(&self) -> Result<[u8; 32], Error> {
        Ok([7; 32])
    }
    // Use real hashing for object checks and original preparation bindings.
    fn hash(&self, bytes: &[u8]) -> Result<[u8; 32], Error> {
        sha256(bytes, bytes.len()).map_err(|_| Error::Limit)
    }
}
struct Fixture {
    path: PathGuard,
    journal: Arc<Journal>,
    store: Arc<Store>,
    completion: Completion,
    token: Option<Blob>,
}
// Keep actual byte/metadata limits finite and independently configurable at recovery.
fn image_limits() -> ImageLimits {
    ImageLimits {
        bytes: 16384,
        leases: 4,
        operations: 8,
        values: Budget {
            bytes: 1024,
            nodes: 64,
            depth: 8,
        },
    }
}
// One pending operation per owner makes lost or unreconstructed holds observable.
fn limits() -> Limits {
    Limits {
        bytes: 16,
        leases: 4,
        operations: 1,
        result_bytes: 4096,
        per_owner: 1,
    }
}
// Preserve trusted original peer scope; a second stream is a distinct attempt, never a remote replay key.
fn key(stream: u32) -> OperationKey {
    OperationKey {
        requester: IdentityId([2; 32]),
        connection: [3; 32],
        stream,
    }
}
// Model monotonic elapsed time independently from later wall-clock jumps.
fn time(elapsed: u64) -> Time {
    Time {
        unix_seconds: 1000 + elapsed,
        elapsed_seconds: elapsed,
        boot: [1; 16],
    }
}
// Create a fully durable byte proposal and optional original preparation before simulating owner reopen.
fn fixture(prepared: bool) -> Fixture {
    let path = path();
    let store = Arc::new(Store::open(&path.0, true).unwrap());
    let crypto = Arc::new(CryptoPort);
    let journal = Arc::new(
        Journal::initialize(
            IdentityId([1; 32]),
            store.clone(),
            crypto,
            image_limits(),
            Anchor::new(time(0)),
        )
        .unwrap(),
    );
    let hash = sha256(b"abc", 3).unwrap();
    let lease = Lease {
        id: [8; 32],
        requester: key(1).requester,
        provider: IdentityId([1; 32]),
        accepted_terms: Blob::new(b"terms".to_vec(), 5).unwrap(),
        size_bytes: 3,
        content_hash: hash,
        accepted_at: 1000,
        expires_at: 1010,
        store_price: 7,
        state: State::Active,
    };
    let mut upload = store.begin(lease.id, 3, 3).unwrap();
    upload.append(0, b"abc").unwrap();
    drop(upload.finish(hash).unwrap());
    let effect = Effect::Create(lease).encode(image_limits()).unwrap();
    let completion = Completion {
        output: Value::Uint64(7),
        result: Blob::new(vec![1], 1).unwrap(),
        effect: Blob::new(effect, image_limits().bytes).unwrap(),
        price: 7,
    };
    journal.stage(key(1), [4; 32], completion.clone()).unwrap();
    let token = if prepared {
        Some(journal.prepare(key(1), &completion.effect).unwrap())
    } else {
        None
    };
    Fixture {
        path,
        journal,
        store,
        completion,
        token,
    }
}
struct Resolver<'a> {
    gate: &'a Gate,
    owner: IdentityId,
    commit: Option<bool>,
    token: Option<Blob>,
    lie: bool,
    calls: AtomicUsize,
}
impl Resume for Resolver<'_> {
    // Model the coordinator's trusted public identity without reading its private journal.
    fn local_identity(&self) -> IdentityId {
        self.owner
    }
    // Deliver only this fixture's explicit decision; absence remains unavailable rather than becoming abort.
    fn resume(&self, key: OperationKey) -> Result<Phase, Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.lie {
            return Ok(Phase::Committed);
        }
        match self.commit {
            Some(true) => {
                let completion = self.gate.completion(key)?;
                let token = self.gate.prepare(key, &completion.effect)?;
                self.gate.commit(key, &token)?;
                Ok(Phase::Committed)
            }
            Some(false) => {
                self.gate.abort(key, self.token.as_ref())?;
                Ok(Phase::Aborted)
            }
            None => Err(Error::Unavailable),
        }
    }
}
// Reuse a fixture's explicitly recorded decision, never an invented timeout rollback.
fn resolver(gate: &Gate, token: Option<Blob>, commit: Option<bool>) -> Resolver<'_> {
    Resolver {
        gate,
        owner: IdentityId([1; 32]),
        commit,
        token,
        lie: false,
        calls: AtomicUsize::new(0),
    }
}
// Actual file/metadata reopen reconstructs holds and advances only after original decision delivery.
#[test]
fn real_reopen_commit_and_quota_reconstruction() {
    let Fixture {
        path,
        journal,
        store,
        completion: _,
        token,
    } = fixture(true);
    drop(journal);
    drop(store);
    let store = Arc::new(Store::open(&path.0, false).unwrap());
    let journal = Arc::new(
        Journal::open(
            IdentityId([1; 32]),
            store.clone(),
            Arc::new(CryptoPort),
            image_limits(),
        )
        .unwrap(),
    );
    let gate = Gate::new(journal.clone(), store, limits()).unwrap();
    assert_eq!(gate.is_ready(), Ok(false));
    assert_eq!(gate.admit(|_, _| Ok(())), Err(Error::Unavailable));
    let authority = resolver(&gate, token.clone(), Some(true));
    gate.recover(&authority, time(1), None).unwrap();
    assert_eq!(authority.calls.load(Ordering::SeqCst), 1);
    assert_eq!(gate.is_ready(), Ok(true));
    assert!(matches!(gate.status(key(1)), Ok(Some(Stage::Committed(_)))));
    assert_eq!(journal.snapshot().unwrap().leases.len(), 1);
    assert_eq!(
        gate.admit(|_, quota| quota.reserve(
            key(2),
            Usage {
                bytes: 14,
                ..Usage::default()
            }
        )),
        Err(Error::Limit)
    );
    gate.admit(|_, quota| {
        quota.reserve(
            key(2),
            Usage {
                bytes: 13,
                ..Usage::default()
            },
        )?;
        quota.transition(key(2), Usage::default())
    })
    .unwrap();
    gate.commit(key(1), token.as_ref().unwrap()).unwrap();
    assert_eq!(gate.is_ready(), Ok(true));
    gate.recover(&authority, time(2), None).unwrap();
    assert_eq!(authority.calls.load(Ordering::SeqCst), 2);
    // Family commitment alone cannot prove the coordinator completed its other participants.
    let unavailable = resolver(&gate, token, None);
    assert_eq!(
        gate.recover(&unavailable, time(3), None),
        Err(Error::Unavailable)
    );
    assert_eq!(gate.is_ready(), Ok(false));
    assert!(matches!(gate.status(key(1)), Ok(Some(Stage::Committed(_)))));
}
// Unavailable or inconsistent coordinator replies leave original preparations and admission unchanged.
#[test]
fn missing_and_foreign_coordinator_never_release_work() {
    let f = fixture(true);
    let gate = Gate::new(f.journal.clone(), f.store.clone(), limits()).unwrap();
    let unavailable = resolver(&gate, f.token.clone(), None);
    assert_eq!(
        gate.recover(&unavailable, time(1), None),
        Err(Error::Unavailable)
    );
    assert_eq!(gate.is_ready(), Ok(false));
    assert!(matches!(gate.status(key(1)), Ok(Some(Stage::Prepared(_)))));
    let mut foreign = resolver(&gate, f.token.clone(), Some(true));
    foreign.owner = IdentityId([9; 32]);
    assert_eq!(gate.recover(&foreign, time(1), None), Err(Error::Denied));
    assert_eq!(foreign.calls.load(Ordering::SeqCst), 0);
    let mut lying = resolver(&gate, f.token.clone(), Some(true));
    lying.lie = true;
    assert_eq!(gate.recover(&lying, time(1), None), Err(Error::Unknown));
    assert_eq!(gate.is_ready(), Ok(false));
    let abort = resolver(&gate, f.token.clone(), Some(false));
    gate.recover(&abort, time(1), None).unwrap();
    assert_eq!(gate.is_ready(), Ok(true));
    assert!(f.journal.snapshot().unwrap().leases.is_empty());
    assert!(matches!(
        gate.status(key(1)),
        Ok(Some(Stage::Aborted(Some(_))))
    ));
}
// A durable but not-yet-prepared effect is recovered through the original coordinator, never executed again.
#[test]
fn staged_and_corrupt_active_recovery() {
    let f = fixture(false);
    let gate = Gate::new(f.journal.clone(), f.store.clone(), limits()).unwrap();
    let authority = resolver(&gate, None, Some(true));
    gate.recover(&authority, time(1), None).unwrap();
    let object = f.path.0.join(format!("object-{}", "08".repeat(32)));
    std::fs::write(object, b"bad").unwrap();
    assert_eq!(gate.recover(&authority, time(2), None), Err(Error::Storage));
    assert_eq!(gate.is_ready(), Ok(false));
    assert_eq!(f.journal.snapshot().unwrap().leases.len(), 1);
}
// Reboot ambiguity, insufficient quotas and unresolved receiving entries block new work without dropping obligations.
#[test]
fn clock_capacity_and_receiving_fail_closed() {
    let f = fixture(true);
    f.journal.commit(key(1), f.token.as_ref().unwrap()).unwrap();
    let small = Gate::new(
        f.journal.clone(),
        f.store.clone(),
        Limits {
            bytes: 2,
            ..limits()
        },
    )
    .unwrap();
    let authority = resolver(&small, None, Some(true));
    assert_eq!(small.recover(&authority, time(1), None), Err(Error::Limit));
    assert_eq!(small.is_ready(), Ok(false));
    let gate = Gate::new(f.journal.clone(), f.store.clone(), limits()).unwrap();
    let authority = resolver(&gate, None, Some(true));
    let boot = Time {
        boot: [2; 16],
        elapsed_seconds: 0,
        ..time(2)
    };
    assert_eq!(
        gate.recover(&authority, boot, None),
        Err(Error::Unavailable)
    );
    assert_eq!(gate.is_ready(), Ok(false));
    gate.recover(&authority, boot, Some(10)).unwrap();
    assert_eq!(
        f.journal.snapshot().unwrap().leases[0].state,
        State::Expired
    );
    let pending = f.store.begin([6; 32], 3, 3).unwrap();
    assert_eq!(
        gate.recover(&authority, boot, None),
        Err(Error::Unavailable)
    );
    assert_eq!(gate.is_ready(), Ok(false));
    drop(pending);
    gate.recover(
        &authority,
        Time {
            unix_seconds: 1,
            ..boot
        },
        None,
    )
    .unwrap();
    assert_eq!(
        f.journal.snapshot().unwrap().leases[0].state,
        State::Expired
    );
}
// Live acknowledged decisions release pending-work capacity while keeping durable result/history bytes charged.
#[test]
fn live_terminal_capacity_and_unknown_admission() {
    let f = fixture(true);
    let gate = Gate::new(f.journal.clone(), f.store.clone(), limits()).unwrap();
    let authority = resolver(&gate, f.token.clone(), Some(true));
    gate.recover(&authority, time(1), None).unwrap();
    let c = Completion {
        effect: Blob::new(vec![0], 1).unwrap(),
        ..f.completion.clone()
    };
    gate.admit(|journal, quota| {
        quota.reserve(
            key(2),
            Usage {
                operations: 1,
                result_bytes: 10,
                ..Usage::default()
            },
        )?;
        journal.stage(key(2), [5; 32], c.clone()).map(|_| ())
    })
    .unwrap();
    let token = gate.prepare(key(2), &c.effect).unwrap();
    gate.commit(key(2), &token).unwrap();
    gate.admit(|_, quota| {
        quota.reserve(
            key(3),
            Usage {
                operations: 1,
                ..Usage::default()
            },
        )
    })
    .unwrap();
    assert_eq!(
        gate.admit::<()>(|_, _| Err(Error::Unknown)),
        Err(Error::Unknown)
    );
    assert_eq!(gate.is_ready(), Ok(false));
    assert_eq!(gate.admit(|_, _| Ok(())), Err(Error::Unavailable));
}
