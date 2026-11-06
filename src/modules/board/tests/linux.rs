//! Real protected filesystem checks use isolated owner-only directories and complete records.
use super::*;
use std::{
    collections::BTreeMap,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    // Create an owner-only isolated directory without relying on shared ports or persistent user state.
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "talksphere-board-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self(p)
    }
}
impl Drop for Fixture {
    // Delete only this fixture's owned temporary tree after all store descriptors have dropped.
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
// A complete public Board record with no fake runtime capabilities is sufficient for storage checks.
fn record() -> Record {
    Record {
        publisher: crate::modules::identity::api::IdentityId([1; 32]),
        board: br#"{"version":1,"providedServices":[],"wantedServices":[]}"#.to_vec(),
        definitions: BTreeMap::new(),
    }
}
// Durable acknowledged data survives reopen; contention and invalid filesystem objects fail safely.
#[test]
fn protected_store_roundtrip_and_locking() {
    let f = Fixture::new();
    let mut s = FileStore::open(&f.0, Limits::default()).unwrap();
    assert_eq!(s.load(), Ok(None));
    assert!(FileStore::open(&f.0, Limits::default()).is_err());
    s.commit(&record()).unwrap();
    drop(s);
    let mut s = FileStore::open(&f.0, Limits::default()).unwrap();
    assert_eq!(s.load(), Ok(Some(record())));
    std::fs::write(f.0.join("board.bin"), b"truncated").unwrap();
    assert!(s.load().is_err());
    drop(s);
    std::fs::set_permissions(&f.0, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(FileStore::open(&f.0, Limits::default()).is_err());
}
// Symlinks, parent traversal and insecure data modes cannot redirect the protected adapter.
#[test]
fn unsafe_paths_and_file_permissions() {
    let f = Fixture::new();
    let link = f.0.join("link");
    symlink(&f.0, &link).unwrap();
    assert!(FileStore::open(&link, Limits::default()).is_err());
    assert!(FileStore::open(&f.0.join("../bad"), Limits::default()).is_err());
    let mut s = FileStore::open(&f.0, Limits::default()).unwrap();
    s.commit(&record()).unwrap();
    std::fs::set_permissions(
        f.0.join("board.bin"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(s.load().is_err());
}
