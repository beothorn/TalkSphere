//! Placement store tests: insertion, atomic updates, listing, bounds, write failure and native reopen.
use super::*;

const LIMITS: Limits = Limits {
    placements: 8,
    payload_bytes: 100,
};

// A placement with distinct identifier, recipient, acceptance time and payload size.
fn placement(id: u8, recipient: u8, accepted_at: u64, size: usize) -> Placement {
    Placement {
        record: Record {
            id: PlacementId([id; 32]),
            metadata: Metadata {
                message_id: MessageId([id; 32]),
                sender_id: IdentityId([1; 32]),
                recipient_id: IdentityId([recipient; 32]),
                sender_created_at: 5,
                encoding: Encoding::Opaque,
                payload_size: size as u64,
                payload_hash: PayloadHash([id; 32]),
            },
            sender_public_key: [7; 32],
            sender_signature: Signature([8; 64]),
            accepted_at,
            expires_at: accepted_at + 10,
            state: State::Pending,
            payments: 17,
            receipt: None,
            removal: None,
        },
        payload: vec![id; size],
    }
}

// A fresh directory path for native stores.
fn dir() -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(0);
    std::env::temp_dir().join(format!(
        "ts-messaging-store-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

// Both backings behave identically for the contract tests below.
fn stores() -> Vec<Store> {
    vec![
        Store::volatile(LIMITS),
        Store::open(&dir(), LIMITS).unwrap(),
    ]
}

// Insertion returns exactly what was stored, record and payload, and a placement is never replaced.
#[test]
fn placements_are_stored_exactly_and_never_replaced() {
    for store in stores() {
        let p = placement(1, 3, 1000, 3);
        store.insert(&p).unwrap();
        assert_eq!(store.get(p.record.id).unwrap().unwrap(), p);
        assert_eq!(store.record(p.record.id), Some(p.record.clone()));
        assert_eq!(store.get(PlacementId([9; 32])).unwrap(), None);
        let mut other = placement(1, 3, 2000, 4);
        other.record.payments = 99;
        assert_eq!(store.insert(&other), Err(Error::Conflict));
        assert_eq!(
            store.get(p.record.id).unwrap().unwrap(),
            p,
            "the original is untouched"
        );
    }
}

// The placement count and the active payload bytes are bounded; reclaimed payloads free their bytes.
#[test]
fn finite_bounds_are_enforced() {
    for store in stores() {
        store.insert(&placement(1, 3, 1, 60)).unwrap();
        assert_eq!(store.insert(&placement(2, 3, 1, 41)), Err(Error::Limit));
        store.insert(&placement(2, 3, 1, 40)).unwrap();
        // A removed placement no longer owes its payload bytes.
        store
            .update(PlacementId([1; 32]), &mut |r| {
                r.state = State::Removed;
                Ok(())
            })
            .unwrap();
        store.release_payload(PlacementId([1; 32])).unwrap();
        store.insert(&placement(3, 3, 1, 60)).unwrap();
        let small = Store::volatile(Limits {
            placements: 2,
            payload_bytes: 1000,
        });
        small.insert(&placement(1, 3, 1, 1)).unwrap();
        small.insert(&placement(2, 3, 1, 1)).unwrap();
        assert_eq!(small.insert(&placement(3, 3, 1, 1)), Err(Error::Limit));
    }
}

// An update applies atomically, may be refused by its closure, never changes identity, metadata or signature, and
// reports unknown placements as not available.
#[test]
fn updates_are_atomic_and_protect_immutable_fields() {
    for store in stores() {
        let p = placement(1, 3, 1000, 3);
        store.insert(&p).unwrap();
        let id = p.record.id;
        let after = store
            .update(id, &mut |r| {
                r.expires_at = 1014;
                r.payments += 6;
                r.metadata.recipient_id = IdentityId([9; 32]);
                r.sender_signature = Signature([0; 64]);
                Ok(())
            })
            .unwrap();
        assert_eq!((after.expires_at, after.payments), (1014, 23));
        assert_eq!(after.metadata, p.record.metadata);
        assert_eq!(after.sender_signature, p.record.sender_signature);
        assert_eq!(
            store.update(id, &mut |_| Err(Error::Conflict)),
            Err(Error::Conflict)
        );
        assert_eq!(store.record(id).unwrap().expires_at, 1014);
        assert_eq!(
            store.update(PlacementId([9; 32]), &mut |_| Ok(())),
            Err(Error::NotAvailable)
        );
    }
}

// A failed write records nothing on insert and changes nothing on update.
#[test]
fn write_failures_change_nothing() {
    let store = Store::volatile(LIMITS);
    let p = placement(1, 3, 1000, 3);
    store.fail_writes(true);
    assert_eq!(store.insert(&p), Err(Error::Storage));
    assert_eq!(store.record(p.record.id), None);
    store.fail_writes(false);
    store.insert(&p).unwrap();
    store.fail_writes(true);
    assert_eq!(
        store.update(p.record.id, &mut |r| {
            r.expires_at = 5000;
            Ok(())
        }),
        Err(Error::Storage)
    );
    assert_eq!(store.record(p.record.id).unwrap().expires_at, 1010);
    // A native store whose directory vanished fails the same way.
    let path = dir();
    let native = Store::open(&path, LIMITS).unwrap();
    std::fs::remove_dir_all(&path).unwrap();
    assert_eq!(native.insert(&p), Err(Error::Storage));
    assert_eq!(native.record(p.record.id), None);
}

// Pending placements list by `(acceptedAt, placementId)` with an exclusive pair cursor, only for the named recipient, only while pending.
#[test]
fn pending_listing_orders_pages_and_isolates_recipients() {
    for store in stores() {
        for (id, recipient, at) in [
            (3u8, 3u8, 1001u64),
            (1, 3, 1000),
            (2, 3, 1000),
            (4, 5, 1000),
        ] {
            store.insert(&placement(id, recipient, at, 1)).unwrap();
        }
        let c = IdentityId([3; 32]);
        let page = |after, limit| {
            store
                .pending_for(c, after, limit)
                .iter()
                .map(|r| r.id.0[0])
                .collect::<Vec<_>>()
        };
        assert_eq!(page(None, 10), vec![1, 2, 3]);
        assert_eq!(page(None, 2), vec![1, 2]);
        assert_eq!(page(Some((1000, PlacementId([2; 32]))), 2), vec![3]);
        assert_eq!(
            page(Some((1001, PlacementId([3; 32]))), 2),
            Vec::<u8>::new()
        );
        assert_eq!(store.pending_for(IdentityId([5; 32]), None, 10).len(), 1);
        assert!(store.pending_for(IdentityId([9; 32]), None, 10).is_empty());
        store
            .update(PlacementId([1; 32]), &mut |r| {
                r.state = State::Recovered;
                Ok(())
            })
            .unwrap();
        assert_eq!(page(None, 10), vec![2, 3]);
        assert_eq!(store.pending_counts(IdentityId([1; 32]), c), (3, 2));
        assert_eq!(store.records().len(), 4);
    }
}

// A reopened native store returns identical records including receipt and removal, with owner-only files.
#[test]
fn native_stores_persist_everything_across_reopen() {
    use std::os::unix::fs::PermissionsExt;
    let path = dir();
    let store = Store::open(&path, LIMITS).unwrap();
    let p = placement(1, 3, 1000, 3);
    store.insert(&p).unwrap();
    store.insert(&placement(2, 3, 1000, 3)).unwrap();
    let receipt = Receipt {
        recovery_id: super::super::ids::RecoveryId([4; 32]),
        recovered_at: 1005,
        recipient_public_key: [5; 32],
        recipient_signature: Signature([6; 64]),
        provider_signature: Signature([7; 64]),
    };
    store
        .update(p.record.id, &mut |r| {
            r.state = State::Recovered;
            r.receipt = Some(receipt);
            Ok(())
        })
        .unwrap();
    let removal = Removal {
        removal_id: RemovalId([8; 32]),
        release_credits: 5,
        price: 2,
    };
    store
        .update(PlacementId([2; 32]), &mut |r| {
            r.state = State::Removed;
            r.removal = Some(removal);
            Ok(())
        })
        .unwrap();
    store.release_payload(PlacementId([2; 32])).unwrap();
    drop(store);
    let reopened = Store::open(&path, LIMITS).unwrap();
    let first = reopened.get(p.record.id).unwrap().unwrap();
    assert_eq!(
        (first.record.state, first.record.receipt, first.payload),
        (State::Recovered, Some(receipt), p.payload)
    );
    let second = reopened.record(PlacementId([2; 32])).unwrap();
    assert_eq!(
        (second.state, second.removal),
        (State::Removed, Some(removal))
    );
    assert_eq!(
        reopened.get(PlacementId([2; 32])),
        Err(Error::Storage),
        "the released payload is gone"
    );
    for entry in std::fs::read_dir(&path).unwrap().flatten() {
        assert_eq!(
            entry.metadata().unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o700
    );
}

// Corrupt, oversized, mismatched and extra-member files make open fail without repairing or replacing anything; an interrupted
// temporary file is discarded.
#[test]
fn damaged_native_stores_are_refused() {
    let path = dir();
    let store = Store::open(&path, LIMITS).unwrap();
    let p = placement(1, 3, 1000, 3);
    store.insert(&p).unwrap();
    drop(store);
    let record = native::record_path(&path, p.record.id);
    let payload = native::payload_path(&path, p.record.id);
    let good = std::fs::read(&record).unwrap();
    for bad in [
        b"nonsense".to_vec(),
        String::from_utf8(good.clone())
            .unwrap()
            .replace("\"payments\":17", "\"payments\":-1")
            .into_bytes(),
        String::from_utf8(good.clone())
            .unwrap()
            .replace("\"version\":1", "\"version\":1,\"extra\":1")
            .into_bytes(),
        String::from_utf8(good.clone())
            .unwrap()
            .replace("\"pending\"", "\"limbo\"")
            .into_bytes(),
        vec![b' '; 20_000],
    ] {
        std::fs::write(&record, &bad).unwrap();
        assert_eq!(Store::open(&path, LIMITS).err(), Some(Error::Storage));
        assert_eq!(std::fs::read(&record).unwrap(), bad, "nothing is repaired");
    }
    std::fs::write(&record, &good).unwrap();
    // A payload of the wrong size, or a missing one, for an active placement is corruption.
    std::fs::write(&payload, b"toolong").unwrap();
    assert_eq!(Store::open(&path, LIMITS).err(), Some(Error::Storage));
    std::fs::remove_file(&payload).unwrap();
    assert_eq!(Store::open(&path, LIMITS).err(), Some(Error::Storage));
    std::fs::write(&payload, &p.payload).unwrap();
    std::fs::write(path.join("interrupted.tmp"), b"half").unwrap();
    let reopened = Store::open(&path, LIMITS).unwrap();
    assert!(!path.join("interrupted.tmp").exists());
    assert_eq!(reopened.get(p.record.id).unwrap().unwrap(), p);
    // A record file whose name disagrees with its identifier is refused.
    std::fs::copy(&record, native::record_path(&path, PlacementId([9; 32]))).unwrap();
    assert_eq!(Store::open(&path, LIMITS).err(), Some(Error::Storage));
}

// The debug form of a placement never prints payload bytes.
#[test]
fn debug_never_prints_the_payload() {
    let text = format!("{:?}", placement(1, 3, 1000, 3));
    assert!(text.contains("3 bytes") && !text.contains("[1, 1, 1]"));
}
