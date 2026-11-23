//! Hosted placement store tests: insertion, host rules, atomic updates, queries, bounds, write failure and native reopen.
use super::*;

pub(crate) const LIMITS: Limits = Limits {
    placements: 8,
    content_bytes: 100,
};

/// A placement of `size` content bytes for a publication of `publisher`, with the given kind, sequence and acceptance time.
pub(crate) fn placement(
    id: u8,
    publisher: u8,
    kind: Kind,
    sequence: u64,
    accepted_at: u64,
    size: usize,
) -> Placement {
    Placement {
        record: Record {
            id: PlacementId([id; 32]),
            metadata: Metadata {
                publication_id: PublicationId([id; 32]),
                publisher_id: IdentityId([publisher; 32]),
                kind,
                sequence,
                published_at: 5,
                visibility: Visibility::Public,
                audience: vec![],
                content_size: size as u64,
                content_hash: ContentHash([id; 32]),
            },
            publisher_public_key: [7; 32],
            publisher_signature: Signature([8; 64]),
            accepted_at,
            expires_at: accepted_at + 10,
            state: State::Pending,
            payments: 17,
            removal: None,
        },
        content: vec![id; size],
    }
}

// A fresh directory path for native stores.
fn dir() -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(0);
    std::env::temp_dir().join(format!(
        "ts-hosted-store-{}-{}",
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

// Insertion returns exactly what was stored, record and content, and a placement ID is never reused or replaced.
#[test]
fn placements_are_stored_exactly_and_never_replaced() {
    for store in stores() {
        let p = placement(1, 3, Kind::Page, 1, 1000, 3);
        store.insert(&p, 1000).unwrap();
        assert_eq!(store.get(p.record.id).unwrap().unwrap(), p);
        assert_eq!(store.record(p.record.id), Some(p.record.clone()));
        assert_eq!(store.get(PlacementId([9; 32])).unwrap(), None);
        let mut other = placement(1, 3, Kind::Page, 2, 2000, 4);
        other.record.payments = 99;
        assert_eq!(store.insert(&other, 2000), Err(Error::Conflict));
        assert_eq!(store.get(p.record.id).unwrap().unwrap(), p);
        assert_eq!(p.publication().content, vec![1; 3]);
        assert!(!format!("{p:?}").contains("[1, 1"));
    }
}

// HP-03: a second active placement of the same publisher's publication ID, or of a page or post with the same sequence but another
// publication ID, is a conflict; resources (sequence zero) and other publishers are not constrained by the sequence rule.
#[test]
fn duplicate_publications_and_sequences_conflict_atomically() {
    for store in stores() {
        store
            .insert(&placement(1, 3, Kind::Page, 5, 1000, 3), 1000)
            .unwrap();
        // The same publication ID again, even at another placement ID, is a duplicate.
        let mut again = placement(2, 3, Kind::Page, 6, 1000, 3);
        again.record.metadata.publication_id = PublicationId([1; 32]);
        assert_eq!(store.insert(&again, 1000), Err(Error::Conflict));
        // Same kind and sequence, another publication.
        assert_eq!(
            store.insert(&placement(3, 3, Kind::Page, 5, 1000, 3), 1000),
            Err(Error::Conflict)
        );
        // Another kind, another publisher, another sequence and resources are accepted.
        for p in [
            placement(4, 3, Kind::FeedPost, 5, 1000, 3),
            placement(5, 4, Kind::Page, 5, 1000, 3),
            placement(6, 3, Kind::Page, 6, 1000, 3),
            placement(7, 3, Kind::Resource, 0, 1000, 3),
        ] {
            store.insert(&p, 1000).unwrap();
        }
        let second_resource = placement(8, 3, Kind::Resource, 0, 1000, 1);
        store.insert(&second_resource, 1000).unwrap();
    }
    // Once the first placement expires, its sequence and publication ID may be used again.
    for store in stores() {
        store
            .insert(&placement(1, 3, Kind::Page, 5, 1000, 3), 1000)
            .unwrap();
        store
            .insert(&placement(3, 3, Kind::Page, 5, 1010, 3), 1010)
            .unwrap();
    }
}

// Updates replace one whole record, cannot change identity, metadata or signature, and a refusal or write failure keeps the old one.
#[test]
fn updates_are_atomic_and_protect_the_signed_fields() {
    for store in stores() {
        let p = placement(1, 3, Kind::Page, 1, 1000, 3);
        store.insert(&p, 1000).unwrap();
        let updated = store
            .update(p.record.id, &mut |r| {
                r.payments += 5;
                r.metadata.sequence = 99;
                r.publisher_signature = Signature([0; 64]);
                Ok(())
            })
            .unwrap();
        assert_eq!(updated.payments, 22);
        assert_eq!(updated.metadata, p.record.metadata);
        assert_eq!(updated.publisher_signature, p.record.publisher_signature);
        assert_eq!(
            store.update(p.record.id, &mut |_| Err(Error::Conflict)),
            Err(Error::Conflict)
        );
        assert_eq!(store.record(p.record.id).unwrap().payments, 22);
        assert_eq!(
            store.update(PlacementId([9; 32]), &mut |_| Ok(())),
            Err(Error::NotAvailable)
        );
    }
    let store = Store::volatile(LIMITS);
    let p = placement(1, 3, Kind::Page, 1, 1000, 3);
    store.insert(&p, 1000).unwrap();
    store.fail_writes(true);
    assert_eq!(
        store.update(p.record.id, &mut |r| {
            r.payments = 1;
            Ok(())
        }),
        Err(Error::Storage)
    );
    assert_eq!(store.record(p.record.id).unwrap().payments, 17);
    assert_eq!(
        store.insert(&placement(2, 3, Kind::Page, 2, 1000, 3), 1000),
        Err(Error::Storage)
    );
    assert_eq!(store.records().len(), 1);
    store.fail_writes(false);
    store
        .insert(&placement(2, 3, Kind::Page, 2, 1000, 3), 1000)
        .unwrap();
}

// Queries return only active placements of the asked publisher, listed newest first with publication ID as tie-breaker.
#[test]
fn queries_select_active_placements_in_listing_order() {
    for store in stores() {
        for p in [
            placement(1, 3, Kind::FeedPost, 7, 1000, 1),
            placement(2, 3, Kind::FeedPost, 5, 1000, 1),
            placement(3, 4, Kind::FeedPost, 9, 1000, 1),
            placement(4, 3, Kind::Page, 1, 1000, 1),
        ] {
            store.insert(&p, 1000).unwrap();
        }
        // Another placement at the same sequence is refused, so order ties only arise across stores; sort is still by identifier.
        let posts = store.active_of_kind(IdentityId([3; 32]), Kind::FeedPost, 1005);
        let ids: Vec<_> = posts.iter().map(|r| r.id.0[0]).collect();
        assert_eq!(ids, vec![1, 2]);
        assert!(
            store
                .active_of_kind(IdentityId([3; 32]), Kind::FeedPost, 1010)
                .is_empty()
        );
        assert_eq!(store.active_count(IdentityId([3; 32]), 1005), 3);
        assert_eq!(store.active_count(IdentityId([3; 32]), 1010), 0);
        let found = store
            .active_publication(IdentityId([4; 32]), PublicationId([3; 32]), 1005)
            .unwrap();
        assert_eq!(found.id, PlacementId([3; 32]));
        assert!(
            store
                .active_publication(IdentityId([3; 32]), PublicationId([3; 32]), 1005)
                .is_none()
        );
        assert!(
            store
                .active_publication(IdentityId([4; 32]), PublicationId([3; 32]), 1010)
                .is_none()
        );
        store
            .update(PlacementId([3; 32]), &mut |r| {
                r.state = State::Removed;
                Ok(())
            })
            .unwrap();
        assert_eq!(store.active_count(IdentityId([4; 32]), 1005), 0);
    }
}

// The placement count and the content bytes of active placements are bounded, and finished placements stop counting bytes.
#[test]
fn bounds_cover_count_and_active_bytes() {
    let store = Store::volatile(LIMITS);
    store
        .insert(&placement(1, 3, Kind::Resource, 0, 1000, 60), 1000)
        .unwrap();
    assert!(store.has_room(40) && !store.has_room(41));
    assert_eq!(
        store.insert(&placement(2, 3, Kind::Resource, 0, 1000, 41), 1000),
        Err(Error::Limit)
    );
    store
        .update(PlacementId([1; 32]), &mut |r| {
            r.state = State::Removed;
            Ok(())
        })
        .unwrap();
    assert!(store.has_room(100));
    store.release_content(PlacementId([1; 32])).unwrap();
    assert_eq!(
        store.release_content(PlacementId([9; 32])),
        Err(Error::NotAvailable)
    );
    for n in 2..=8 {
        store
            .insert(&placement(n, 3, Kind::Resource, 0, 1000, 1), 1000)
            .unwrap();
    }
    assert!(!store.has_room(1));
    assert_eq!(
        store.insert(&placement(9, 3, Kind::Resource, 0, 1000, 1), 1000),
        Err(Error::Limit)
    );
    // Discarding frees the record slot; discarding again or an unknown placement is a no-op.
    store.discard(PlacementId([1; 32])).unwrap();
    store.discard(PlacementId([1; 32])).unwrap();
    assert!(store.has_room(1));
}

// HP-02: a native store reopened after a restart returns identical records and contents, including restricted audiences and a recorded
// removal; the original files are owner-only.
#[test]
fn native_stores_survive_a_restart() {
    use std::os::unix::fs::PermissionsExt;
    let dir = dir();
    let store = Store::open(&dir, LIMITS).unwrap();
    let mut restricted = placement(1, 3, Kind::FeedPost, 2, 1000, 5);
    restricted.record.metadata.visibility = Visibility::Restricted;
    restricted.record.metadata.audience = vec![IdentityId([4; 32]), IdentityId([5; 32])];
    let removed = placement(2, 3, Kind::Page, 1, 1000, 2);
    store.insert(&restricted, 1000).unwrap();
    store.insert(&removed, 1000).unwrap();
    store
        .update(removed.record.id, &mut |r| {
            r.state = State::Removed;
            r.removal = Some(Removal {
                removal_id: RemovalId([6; 32]),
                release_credits: 4,
                price: 2,
            });
            Ok(())
        })
        .unwrap();
    store.release_content(removed.record.id).unwrap();
    drop(store);
    let reopened = Store::open(&dir, LIMITS).unwrap();
    assert_eq!(
        reopened.get(restricted.record.id).unwrap().unwrap(),
        restricted
    );
    let record = reopened.record(removed.record.id).unwrap();
    assert_eq!(record.state, State::Removed);
    assert_eq!(record.removal.unwrap().release_credits, 4);
    assert_eq!(reopened.records().len(), 2);
    let mode = |name: String| {
        std::fs::metadata(dir.join(name))
            .unwrap()
            .permissions()
            .mode()
            & 0o777
    };
    assert_eq!(mode(format!("{}.json", hex(&[1; 32]))), 0o600);
    assert_eq!(mode(format!("{}.content", hex(&[1; 32]))), 0o600);
    assert_eq!(
        std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
}

// The record file of a placement.
fn record_file(dir: &Path, p: &Placement) -> PathBuf {
    dir.join(format!("{}.json", hex(&p.record.id.0)))
}

/// A way to damage a seeded store directory.
type Corruption = Box<dyn Fn(&Path, &Placement)>;

// A damaged directory is refused without repair: a corrupt or misnamed record, an unknown member, a missing or wrong-sized content,
// too many records; an interrupted temporary write is simply discarded.
#[test]
fn corrupt_native_stores_are_refused() {
    let seed = |dir: &Path| {
        let store = Store::open(dir, LIMITS).unwrap();
        let p = placement(1, 3, Kind::Page, 1, 1000, 3);
        store.insert(&p, 1000).unwrap();
        p
    };
    let corruptions: Vec<Corruption> = vec![
        Box::new(|d, p| std::fs::write(record_file(d, p), b"not json").unwrap()),
        Box::new(|d, p| {
            let text = std::fs::read_to_string(record_file(d, p)).unwrap();
            std::fs::write(
                record_file(d, p),
                text.replace("\"version\":1", "\"version\":2"),
            )
            .unwrap();
        }),
        Box::new(|d, p| {
            let text = std::fs::read_to_string(record_file(d, p)).unwrap();
            std::fs::write(record_file(d, p), text.replace("}", ",\"extra\":1}")).unwrap();
        }),
        Box::new(|d, p| {
            std::fs::remove_file(d.join(format!("{}.content", hex(&p.record.id.0)))).unwrap()
        }),
        Box::new(|d, p| {
            std::fs::write(
                d.join(format!("{}.content", hex(&p.record.id.0))),
                b"toolong",
            )
            .unwrap()
        }),
        Box::new(|d, p| {
            std::fs::rename(record_file(d, p), d.join(format!("{}.json", hex(&[9; 32])))).unwrap()
        }),
        Box::new(|d, p| {
            let big = vec![b' '; MAXIMUM_RECORD_BYTES as usize + 1];
            std::fs::write(record_file(d, p), big).unwrap();
        }),
    ];
    for corrupt in corruptions {
        let dir = dir();
        let p = seed(&dir);
        corrupt(&dir, &p);
        assert_eq!(Store::open(&dir, LIMITS).err(), Some(Error::Storage));
    }
    let dir = dir();
    seed(&dir);
    std::fs::write(dir.join("leftover.tmp"), b"partial").unwrap();
    assert!(Store::open(&dir, LIMITS).is_ok());
    assert!(!dir.join("leftover.tmp").exists());
    let small = Limits {
        placements: 0,
        ..LIMITS
    };
    assert_eq!(Store::open(&dir, small).err(), Some(Error::Storage));
}
