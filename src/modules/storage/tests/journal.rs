//! Participant preparation, durable reply-loss recovery and original-key isolation through public ports.
use super::*;
use crate::modules::{
    cryptography::primitives::hash::sha256,
    services::value::{Budget, Value},
    storage::{
        contract::{ContentHash, LeaseId, Time},
        test_support::{Durable, Store, Write},
    },
};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
struct CryptoPort {
    calls: AtomicUsize,
    fail: AtomicBool,
}
impl Crypto for CryptoPort {
    // Fixed test entropy is explicitly a dependency substitute, never production randomness.
    fn lease_id(&self) -> Result<LeaseId, Error> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(Error::Unavailable);
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok([7; 32])
    }
    // Use actual hashing so different original keys/effects bind independently in preparation tests.
    fn hash(&self, bytes: &[u8]) -> Result<ContentHash, Error> {
        sha256(bytes, bytes.len()).map_err(|_| Error::Limit)
    }
}
// Use finite byte/record limits independently of any fixture's logical request size.
fn limits() -> Limits {
    Limits {
        bytes: 8192,
        leases: 4,
        operations: 4,
        values: Budget {
            depth: 4,
            nodes: 16,
            bytes: 256,
        },
    }
}
// Keep provider and operation requester distinct; reconnects differ by the original connection scope.
fn key(stream: u32) -> OperationKey {
    OperationKey {
        requester: IdentityId([2; 32]),
        connection: [3; 32],
        stream,
    }
}
// Retain actual logical bytes and a fixed positive accepted price, independent of codec construction.
fn completion() -> Completion {
    Completion {
        output: Value::Uint64(u64::MAX),
        result: Blob::new(b"result".to_vec(), 6).unwrap(),
        effect: Blob::new(vec![0], 1).unwrap(),
        price: 7,
    }
}
// Create an explicitly initialized journal and independently observable durable dependency.
fn fixture(actions: Vec<Write>) -> (Journal, Arc<Store>, Arc<CryptoPort>) {
    let store = Arc::new(Store {
        durable: Arc::new(Mutex::new(Durable::default())),
        writes: Mutex::new(actions.into()),
    });
    let crypto = Arc::new(CryptoPort {
        calls: AtomicUsize::new(0),
        fail: AtomicBool::new(false),
    });
    let anchor = Anchor::new(Time {
        unix_seconds: 1000,
        elapsed_seconds: 1,
        boot: [1; 16],
    });
    let journal = Journal::initialize(
        IdentityId([1; 32]),
        store.clone(),
        crypto.clone(),
        limits(),
        anchor,
    )
    .unwrap();
    (journal, store, crypto)
}
// Exact repeats consume neither another stage write nor new token entropy; changed reuse and effects conflict.
#[test]
fn original_key_preparation_and_reopen() {
    let (journal, store, crypto) = fixture(vec![Write::Commit; 8]);
    let c = completion();
    assert!(matches!(journal.status(key(1)), Ok(None)));
    assert!(matches!(
        journal.completion(key(1)),
        Err(Error::Unavailable)
    ));
    journal.stage(key(1), [4; 32], c.clone()).unwrap();
    journal.stage(key(1), [4; 32], c.clone()).unwrap();
    assert!(matches!(journal.status(key(1)), Ok(Some(Stage::Staged))));
    assert!(matches!(
        journal.stage(key(1), [5; 32], c.clone()),
        Err(Error::Conflict)
    ));
    let mut changed = c.clone();
    changed.price = 8;
    assert!(matches!(
        journal.stage(key(1), [4; 32], changed),
        Err(Error::Conflict)
    ));
    assert!(matches!(
        journal.prepare(key(1), &Blob::new(vec![9], 1).unwrap()),
        Err(Error::Conflict)
    ));
    assert!(matches!(
        journal.prepare(key(9), &c.effect),
        Err(Error::Unavailable)
    ));
    let token = journal.prepare(key(1), &c.effect).unwrap();
    assert!(journal.prepare(key(1), &c.effect).unwrap() == token);
    assert_eq!(crypto.calls.load(Ordering::SeqCst), 1);
    assert_eq!(store.durable.lock().unwrap().commits, 3);
    drop(journal);
    let recovered =
        Journal::open(IdentityId([1; 32]), store.clone(), crypto.clone(), limits()).unwrap();
    assert!(matches!(recovered.status(key(1)),Ok(Some(Stage::Prepared(t))) if t==token));
    assert!(recovered.prepare(key(1), &c.effect).unwrap() == token);
    assert!(same_completion(&recovered.completion(key(1)).unwrap(), &c));
    recovered.stage(key(2), [4; 32], c.clone()).unwrap();
    assert!(recovered.prepare(key(2), &c.effect).unwrap() != token);
    assert!(matches!(
        Journal::initialize(
            IdentityId([1; 32]),
            store,
            crypto,
            limits(),
            Anchor::new(Time {
                unix_seconds: 1,
                elapsed_seconds: 1,
                boot: [1; 16]
            })
        ),
        Err(Error::Conflict)
    ));
}
// Lost preparation acknowledgement gates every operation until reload recovers the committed original token.
#[test]
fn uncertain_prepare_requires_reload() {
    let (journal, store, crypto) = fixture(vec![Write::Commit, Write::Commit, Write::LoseReply]);
    let c = completion();
    journal.stage(key(1), [4; 32], c.clone()).unwrap();
    assert!(matches!(
        journal.prepare(key(1), &c.effect),
        Err(Error::Unknown)
    ));
    assert!(matches!(journal.status(key(1)), Err(Error::Unknown)));
    assert!(matches!(journal.completion(key(1)), Err(Error::Unknown)));
    assert!(matches!(
        journal.stage(key(2), [4; 32], c.clone()),
        Err(Error::Unknown)
    ));
    journal.reload().unwrap();
    let token = journal.prepare(key(1), &c.effect).unwrap();
    assert_eq!(token.bytes().len(), 64);
    assert_eq!(crypto.calls.load(Ordering::SeqCst), 1);
    assert_eq!(store.durable.lock().unwrap().commits, 3);
    store.durable.lock().unwrap().bytes = None;
    assert_eq!(journal.reload(), Err(Error::Unknown));
    assert!(matches!(journal.status(key(1)), Err(Error::Unknown)));
}
// Known persistence and entropy failures leave the prior staged image unchanged without pretending preparation.
#[test]
fn dependency_failure_preserves_staged_work() {
    let (journal, store, crypto) = fixture(vec![
        Write::Commit,
        Write::Commit,
        Write::Fail,
        Write::Commit,
    ]);
    let c = completion();
    journal.stage(key(1), [4; 32], c.clone()).unwrap();
    crypto.fail.store(true, Ordering::SeqCst);
    assert!(matches!(
        journal.prepare(key(1), &c.effect),
        Err(Error::Unavailable)
    ));
    assert_eq!(store.durable.lock().unwrap().commits, 2);
    crypto.fail.store(false, Ordering::SeqCst);
    assert!(matches!(
        journal.prepare(key(1), &c.effect),
        Err(Error::Unavailable)
    ));
    assert!(matches!(journal.status(key(1)), Ok(Some(Stage::Staged))));
    journal.prepare(key(1), &c.effect).unwrap();
    let empty = Arc::new(Store {
        durable: Arc::new(Mutex::new(Durable::default())),
        writes: Mutex::new(Vec::new().into()),
    });
    assert!(matches!(
        Journal::open(IdentityId([1; 32]), empty, crypto, limits()),
        Err(Error::Unavailable)
    ));
}
// Competing preparation calls serialize to one token and one durable prepare write.
#[test]
fn concurrent_prepare_is_idempotent() {
    use std::{sync::Barrier, thread};
    let (journal, store, crypto) = fixture(vec![Write::Commit; 4]);
    let c = completion();
    journal.stage(key(1), [4; 32], c.clone()).unwrap();
    let journal = Arc::new(journal);
    let gate = Arc::new(Barrier::new(3));
    let jobs: Vec<_> = (0..2)
        .map(|_| {
            let j = journal.clone();
            let gate = gate.clone();
            let effect = c.effect.clone();
            thread::spawn(move || {
                gate.wait();
                j.prepare(key(1), &effect).unwrap()
            })
        })
        .collect();
    gate.wait();
    let mut tokens = jobs.into_iter().map(|j| j.join().unwrap());
    assert!(tokens.next().unwrap() == tokens.next().unwrap());
    assert_eq!(crypto.calls.load(Ordering::SeqCst), 1);
    assert_eq!(store.durable.lock().unwrap().commits, 3);
}

#[cfg(target_os = "linux")]
// Real protected snapshots retain the same preparation token after reconstructing both adapter and journal.
#[test]
fn real_journal_reopen_and_corruption_gate() {
    use crate::modules::storage::bytes::{Store as ByteStore, tests::path};
    let p = path();
    let store = Arc::new(ByteStore::open(&p.0, true).unwrap());
    let crypto = Arc::new(CryptoPort {
        calls: AtomicUsize::new(0),
        fail: AtomicBool::new(false),
    });
    let anchor = Anchor::new(Time {
        unix_seconds: 1000,
        elapsed_seconds: 1,
        boot: [1; 16],
    });
    let journal = Journal::initialize(
        IdentityId([1; 32]),
        store.clone(),
        crypto.clone(),
        limits(),
        anchor,
    )
    .unwrap();
    let c = completion();
    journal.stage(key(1), [4; 32], c.clone()).unwrap();
    let token = journal.prepare(key(1), &c.effect).unwrap();
    drop(journal);
    drop(store);
    let store = Arc::new(ByteStore::open(&p.0, false).unwrap());
    let recovered =
        Journal::open(IdentityId([1; 32]), store.clone(), crypto.clone(), limits()).unwrap();
    assert!(matches!(recovered.status(key(1)),Ok(Some(Stage::Prepared(t))) if t==token));
    assert!(recovered.prepare(key(1), &c.effect).unwrap() == token);
    assert!(matches!(
        Journal::open(IdentityId([9; 32]), store, crypto, limits()),
        Err(Error::Denied)
    ));
    std::fs::write(p.0.join("metadata.bin"), b"corrupt").unwrap();
    assert_eq!(recovered.reload(), Err(Error::Invalid));
    assert!(matches!(recovered.status(key(1)), Err(Error::Unknown)));
}

// Construct a complete owner proposal independently of journal mutation helpers; bytes remain a separate participant concern.
fn lease() -> crate::modules::storage::contract::Lease {
    use crate::modules::storage::contract::{Lease, State};
    Lease {
        id: [8; 32],
        requester: IdentityId([2; 32]),
        provider: IdentityId([1; 32]),
        accepted_terms: Blob::new(b"terms".to_vec(), 5).unwrap(),
        size_bytes: 3,
        content_hash: [9; 32],
        accepted_at: 1000,
        expires_at: 1010,
        store_price: 7,
        state: State::Active,
    }
}
// Encode an exact family-owned proposal into the retained completion's opaque effect reference.
fn proposed(effect: crate::modules::storage::effect::Effect) -> Completion {
    let mut c = completion();
    c.effect = Blob::new(effect.encode(limits()).unwrap(), limits().bytes).unwrap();
    c
}
// Observe installed image through the public persistence contract, without accessing journal locks or tables.
fn installed(store: &Store) -> Image {
    Image::decode(
        &store.load(limits().bytes).unwrap().unwrap(),
        IdentityId([1; 32]),
        limits(),
    )
    .unwrap()
}

// Create, renew and remove decisions each apply once; original tokens remain valid through terminal response loss.
#[test]
fn exact_commit_and_serialized_mutations() {
    use crate::modules::storage::{contract::State, effect::Effect};
    let (journal, store, _) = fixture(vec![Write::Commit; 24]);
    let original = lease();
    let c = proposed(Effect::Create(original.clone()));
    journal.stage(key(1), [4; 32], c.clone()).unwrap();
    let wrong = Blob::new(vec![1], 1).unwrap();
    assert_eq!(journal.commit(key(1), &wrong), Err(Error::Conflict));
    let token = journal.prepare(key(1), &c.effect).unwrap();
    assert_eq!(journal.commit(key(1), &wrong), Err(Error::Conflict));
    journal.commit(key(1), &token).unwrap();
    let writes = store.durable.lock().unwrap().commits;
    journal.commit(key(1), &token).unwrap();
    assert_eq!(store.durable.lock().unwrap().commits, writes);
    assert!(journal.prepare(key(1), &c.effect).unwrap() == token);
    assert_eq!(journal.abort(key(1), Some(&token)), Err(Error::Conflict));
    assert_eq!(installed(&store).leases.len(), 1);
    let mut renewed = original.clone();
    renewed.expires_at = 1014;
    let c = proposed(Effect::Renew {
        expected_expires_at: 1010,
        lease: renewed.clone(),
    });
    journal.stage(key(2), [5; 32], c.clone()).unwrap();
    let token = journal.prepare(key(2), &c.effect).unwrap();
    journal.commit(key(2), &token).unwrap();
    journal.commit(key(2), &token).unwrap();
    let state = installed(&store);
    assert_eq!(state.leases[0].expires_at, 1014);
    assert_eq!(state.leases[0].content_hash, original.content_hash);
    assert_eq!(state.leases[0].accepted_at, original.accepted_at);
    let removal = proposed(Effect::Remove {
        id: original.id,
        expected_expires_at: 1014,
    });
    journal.stage(key(3), [6; 32], removal.clone()).unwrap();
    let token = journal.prepare(key(3), &removal.effect).unwrap();
    renewed.expires_at = 1018;
    let competing = proposed(Effect::Renew {
        expected_expires_at: 1014,
        lease: renewed,
    });
    journal.stage(key(4), [7; 32], competing.clone()).unwrap();
    assert!(matches!(
        journal.prepare(key(4), &competing.effect),
        Err(Error::Conflict)
    ));
    journal.commit(key(3), &token).unwrap();
    assert_eq!(installed(&store).leases[0].state, State::Removed);
    assert!(matches!(
        journal.prepare(key(4), &competing.effect),
        Err(Error::Conflict)
    ));
    journal.abort(key(4), None).unwrap();
}
// Staged/prepared aborts release no committed mutation and retain exact token validation for repeated decisions.
#[test]
fn aborts_preserve_original_token_and_forbid_reactivation() {
    let (journal, store, _) = fixture(vec![Write::Commit; 12]);
    let c = completion();
    let wrong = Blob::new(vec![1], 1).unwrap();
    journal.stage(key(1), [4; 32], c.clone()).unwrap();
    assert_eq!(journal.abort(key(1), Some(&wrong)), Err(Error::Conflict));
    journal.abort(key(1), None).unwrap();
    let writes = store.durable.lock().unwrap().commits;
    journal.abort(key(1), None).unwrap();
    assert_eq!(store.durable.lock().unwrap().commits, writes);
    assert_eq!(journal.abort(key(1), Some(&wrong)), Err(Error::Conflict));
    assert!(matches!(journal.completion(key(1)), Err(Error::Conflict)));
    assert!(matches!(
        journal.stage(key(1), [4; 32], c.clone()),
        Err(Error::Conflict)
    ));
    assert!(matches!(
        journal.prepare(key(1), &c.effect),
        Err(Error::Conflict)
    ));
    journal.stage(key(2), [5; 32], c.clone()).unwrap();
    let token = journal.prepare(key(2), &c.effect).unwrap();
    assert_eq!(journal.abort(key(2), None), Err(Error::Conflict));
    assert_eq!(journal.abort(key(2), Some(&wrong)), Err(Error::Conflict));
    journal.abort(key(2), Some(&token)).unwrap();
    journal.abort(key(2), Some(&token)).unwrap();
    assert_eq!(journal.abort(key(2), Some(&wrong)), Err(Error::Conflict));
    assert_eq!(journal.commit(key(2), &token), Err(Error::Conflict));
    assert_eq!(journal.commit(key(9), &wrong), Err(Error::Unavailable));
    assert_eq!(journal.abort(key(9), None), Err(Error::Unavailable));
}
// A lost terminal acknowledgement never reapplies a create or guesses that committed work was aborted.
#[test]
fn uncertain_terminal_decision_recovers_once() {
    use crate::modules::storage::effect::Effect;
    for commit in [true, false] {
        let (journal, store, _) = fixture(vec![
            Write::Commit,
            Write::Commit,
            Write::Commit,
            Write::LoseReply,
        ]);
        let c = proposed(Effect::Create(lease()));
        journal.stage(key(1), [4; 32], c.clone()).unwrap();
        let token = journal.prepare(key(1), &c.effect).unwrap();
        let result = if commit {
            journal.commit(key(1), &token)
        } else {
            journal.abort(key(1), Some(&token))
        };
        assert_eq!(result, Err(Error::Unknown));
        assert!(matches!(journal.status(key(1)), Err(Error::Unknown)));
        journal.reload().unwrap();
        if commit {
            journal.commit(key(1), &token).unwrap();
            assert_eq!(installed(&store).leases.len(), 1);
        } else {
            journal.abort(key(1), Some(&token)).unwrap();
            assert!(installed(&store).leases.is_empty());
        }
        assert_eq!(store.durable.lock().unwrap().commits, 4);
    }
}
