//! Real filesystem upload protection, exact ranges and durability cut tests.
use super::*;
use std::io::Write;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
const HASH: [u8; 32] = [
    0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22, 0x23,
    0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00, 0x15, 0xad,
];
pub(crate) struct PathGuard(pub(crate) PathBuf);
impl Drop for PathGuard {
    // Remove isolated test artifacts even if an assertion fails; production objects are never touched.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
// Allocate an absent unique directory so every case owns its lock and artifacts.
pub(crate) fn path() -> PathGuard {
    PathGuard(std::env::temp_dir().join(format!(
        "talksphere-storage-bytes-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    )))
}
// Construct complete abc bytes from independently ordered caller chunks.
fn upload(store: &Store, id: [u8; 32]) -> Upload {
    let mut u = store.begin(id, 3, 3).unwrap();
    u.append(0, b"a").unwrap();
    u.append(1, b"bc").unwrap();
    u
}
// Exact durable content reopens after all previous ownership is dropped; readers have independent positions.
#[test]
fn durable_reopen_ranges_and_collision() {
    let p = path();
    assert!(Store::open(&p.0, false).is_err());
    assert!(!p.0.exists());
    let store = Store::open(&p.0, true).unwrap();
    assert!(matches!(Store::open(&p.0, false), Err(Error::Denied)));
    let handle = upload(&store, [1; 32]).finish(HASH).unwrap();
    assert_eq!(handle.id(), [1; 32]);
    assert_eq!(handle.len(), 3);
    assert!(!handle.is_empty());
    assert_eq!(handle.read(1, 2, 2), Ok(b"bc".to_vec()));
    assert_eq!(handle.clone().read(0, 1, 1), Ok(b"a".to_vec()));
    for (offset, length, error) in [
        (0, 0, Error::Invalid),
        (3, 1, Error::Invalid),
        (u64::MAX, 1, Error::Invalid),
        (2, 2, Error::Invalid),
        (0, 3, Error::Limit),
    ] {
        assert_eq!(handle.read(offset, length, 2), Err(error));
    }
    assert!(matches!(
        upload(&store, [1; 32]).finish(HASH),
        Err(Error::Conflict)
    ));
    assert_eq!(handle.read(0, 3, 3), Ok(b"abc".to_vec()));
    drop(handle);
    drop(store);
    let store = Store::open(&p.0, false).unwrap();
    assert_eq!(
        store.reopen([1; 32], 3).unwrap().read(0, 3, 3),
        Ok(b"abc".to_vec())
    );
    assert!(matches!(store.reopen([1; 32], 2), Err(Error::Invalid)));
    assert!(store.reopen([9; 32], 3).is_err());
}
// Invalid uploads never install a complete object, and over-limit declarations allocate no data file.
#[test]
fn incomplete_corrupt_and_poisoned_uploads() {
    let p = path();
    let store = Store::open(&p.0, true).unwrap();
    assert!(matches!(store.begin([1; 32], 0, 3), Err(Error::Invalid)));
    assert!(matches!(
        store.begin([1; 32], u64::MAX, 3),
        Err(Error::Limit)
    ));
    let pending = store.begin([1; 32], 3, 3).unwrap();
    assert!(matches!(store.begin([1; 32], 3, 3), Err(Error::Conflict)));
    drop(pending);
    assert!(matches!(
        store.begin([1; 32], 3, 3).unwrap().finish(HASH),
        Err(Error::Invalid)
    ));
    assert!(matches!(
        upload(&store, [1; 32]).finish([0; 32]),
        Err(Error::Invalid)
    ));
    for (offset, chunk) in [(1, &b"a"[..]), (0, &b"abcd"[..])] {
        let mut u = store.begin([1; 32], 3, 3).unwrap();
        assert_eq!(u.append(offset, chunk), Err(Error::Invalid));
        assert_eq!(u.append(0, b"abc"), Err(Error::Invalid));
        assert!(matches!(u.finish(HASH), Err(Error::Invalid)));
    }
    assert!(store.reopen([1; 32], 3).is_err());
    // Reproduce a crashed writer's provisional entry without retaining live process handles.
    let mut orphan = File::from(
        fs::openat(
            &store.0.file,
            name([2; 32], true),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL,
            Mode::RUSR | Mode::WUSR,
        )
        .unwrap(),
    );
    orphan.write_all(b"a").unwrap();
    orphan.sync_all().unwrap();
    drop(orphan);
    store.discard([2; 32]).unwrap();
    store.discard([2; 32]).unwrap();
    assert!(!p.0.join(name([2; 32], true)).exists());
}
// Cut points distinguish confirmed pre-installation failure from installed bytes with an uncertain reply.
#[test]
fn write_and_sync_cut_matrix() {
    for point in 1..=5 {
        let p = path();
        let store = Store::open(&p.0, true).unwrap();
        let mut u = store.begin([1; 32], 3, 3).unwrap();
        if point == 1 {
            u.fault = Some(1);
            assert_eq!(u.append(0, b"abc"), Err(Error::Storage));
            assert_eq!(u.append(0, b"abc"), Err(Error::Invalid));
            drop(u);
        } else {
            u.append(0, b"abc").unwrap();
            u.fault = Some(point);
            assert!(
                matches!(u.finish(HASH),Err(e) if e==if point>=4{Error::Unknown}else{Error::Storage})
            );
        }
        drop(store);
        let reopened = Store::open(&p.0, false).unwrap();
        if point >= 4 {
            assert_eq!(
                reopened.reopen([1; 32], 3).unwrap().read(0, 3, 3),
                Ok(b"abc".to_vec())
            );
        } else {
            assert!(reopened.reopen([1; 32], 3).is_err());
        }
    }
}
// Unsafe entry types and modes fail without following symlinks, and accepted handles survive later unlink.
#[test]
fn protected_paths_and_retained_handles() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let p = path();
    let store = Store::open(&p.0, true).unwrap();
    let h = upload(&store, [1; 32]).finish(HASH).unwrap();
    std::fs::remove_file(p.0.join(name([1; 32], false))).unwrap();
    assert_eq!(h.read(0, 3, 3), Ok(b"abc".to_vec()));
    let target = p.0.join("target");
    std::fs::write(&target, b"abc").unwrap();
    symlink(&target, p.0.join(name([2; 32], false))).unwrap();
    assert!(store.reopen([2; 32], 3).is_err());
    symlink(&target, p.0.join(name([2; 32], true))).unwrap();
    assert!(store.discard([2; 32]).is_err());
    let mut u = store.begin([3; 32], 3, 3).unwrap();
    u.append(0, b"abc").unwrap();
    let h2 = u.finish(HASH).unwrap();
    std::fs::set_permissions(
        p.0.join(name([3; 32], false)),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(matches!(store.reopen([3; 32], 3), Err(Error::Denied)));
    std::fs::OpenOptions::new()
        .write(true)
        .open(p.0.join(name([3; 32], false)))
        .unwrap()
        .set_len(1)
        .unwrap();
    assert_eq!(h2.read(0, 3, 3), Err(Error::Storage));
    drop(h2);
    drop(h);
    drop(store);
    std::fs::set_permissions(&p.0, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(Store::open(&p.0, false), Err(Error::Denied)));
    let link = path();
    symlink(&p.0, &link.0).unwrap();
    assert!(Store::open(&link.0, false).is_err());
    std::fs::remove_file(&link.0).unwrap();
}

// Recovery inventories every canonical owner entry without following links or ignoring malformed files.
#[test]
fn bounded_inventory_identifies_provisional_and_complete_bytes() {
    let p = path();
    let store = Store::open(&p.0, true).unwrap();
    assert_eq!(store.inventory(0), Ok(vec![]));
    let mut u = store.begin([0xab; 32], 3, 3).unwrap();
    u.append(0, b"a").unwrap();
    assert_eq!(
        store.inventory(1),
        Ok(vec![Entry {
            id: [0xab; 32],
            length: 1,
            pending: true
        }])
    );
    assert_eq!(store.inventory(0), Err(Error::Limit));
    u.append(1, b"bc").unwrap();
    drop(u.finish(HASH).unwrap());
    assert_eq!(
        store.inventory(1),
        Ok(vec![Entry {
            id: [0xab; 32],
            length: 3,
            pending: false
        }])
    );
    let bad = p.0.join(format!("object-{}", "AA".repeat(32)));
    std::fs::write(&bad, b"bad").unwrap();
    assert_eq!(store.inventory(2), Err(Error::Invalid));
    std::fs::remove_file(bad).unwrap();
    let unknown = p.0.join("unexpected");
    std::fs::write(&unknown, b"bad").unwrap();
    assert_eq!(store.inventory(2), Err(Error::Invalid));
}
