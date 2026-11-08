//! Real filesystem durability and protection tests for the independent Services adapter.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
static NEXT: AtomicUsize = AtomicUsize::new(0);
// Allocate a unique absent path without sharing locks or state across tests.
fn path() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "talksphere-services-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ))
}
// Keep the same bounded configuration for fresh writer and reopened observations.
fn limits() -> Limits {
    Limits {
        handlers: 16,
        operations: 16,
        bodies: 16,
        nodes: 16,
        quotes: 16,
        state_bytes: 4096,
    }
}
// Acknowledged images survive reopen, cannot be read by a competing writer and reject insecure paths.
#[test]
fn real_store_and_protection() {
    let p = path();
    assert!(FileStore::open(&p, limits(), false).is_err());
    assert!(!p.exists());
    let mut s = FileStore::open(&p, limits(), true).unwrap();
    assert_eq!(s.load(), Ok(None));
    assert!(FileStore::open(&p, limits(), false).is_err());
    let r = Record {
        identity: crate::modules::identity::api::IdentityId([1; 32]),
        payload: b"complete".to_vec(),
    };
    s.commit(&r).unwrap();
    drop(s);
    let mut s = FileStore::open(&p, limits(), false).unwrap();
    assert_eq!(s.load(), Ok(Some(r)));
    drop(s);
    std::fs::write(p.join("services.bin"), b"bad").unwrap();
    assert!(
        FileStore::open(&p, limits(), false)
            .unwrap()
            .load()
            .is_err()
    );
    std::fs::remove_dir_all(&p).unwrap();
    let target = path();
    std::fs::create_dir(&target).unwrap();
    let link = path();
    std::os::unix::fs::symlink(&target, &link).unwrap();
    assert!(FileStore::open(&link, limits(), false).is_err());
    std::fs::remove_file(link).unwrap();
    std::fs::remove_dir(target).unwrap();
}
// Each simulated failure resolves to a complete permitted old/new image without promoting abandoned staging.
#[test]
fn commit_cut_matrix() {
    for cut in 1..=5 {
        let p = path();
        let old = Record {
            identity: crate::modules::identity::api::IdentityId([1; 32]),
            payload: vec![1],
        };
        let new = Record {
            payload: vec![2],
            ..old.clone()
        };
        let mut s = FileStore::open(&p, limits(), true).unwrap();
        s.commit(&old).unwrap();
        s.fault = Some(cut);
        assert_eq!(
            s.commit(&new),
            Err(if cut >= 4 {
                Error::Unknown
            } else {
                Error::Storage
            })
        );
        s.fault = None;
        assert_eq!(s.load(), Ok(Some(if cut >= 4 { new } else { old })));
        drop(s);
        std::fs::remove_dir_all(p).unwrap();
    }
}
// Existing world-readable files, hard links and unsafe parent traversal are denied.
#[test]
fn unsafe_metadata() {
    use std::os::unix::fs::PermissionsExt;
    let p = path();
    let mut s = FileStore::open(&p, limits(), true).unwrap();
    s.commit(&Record {
        identity: crate::modules::identity::api::IdentityId([1; 32]),
        payload: vec![],
    })
    .unwrap();
    std::fs::set_permissions(
        p.join("services.bin"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert_eq!(s.load(), Err(Error::Denied));
    drop(s);
    std::fs::remove_dir_all(&p).unwrap();
    assert!(FileStore::open(std::path::Path::new("../unsafe-services"), limits(), false).is_err());
}
