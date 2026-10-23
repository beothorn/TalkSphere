//! Protected filesystem tests use fresh temporary stores and published test-only key material.
use super::file::*;
use crate::modules::{
    cryptography::{identity_keys::IdentityKeys, test_support::ScriptedEntropy},
    identity::{authority::Authority, config::Limits, record::CredentialRecord},
};
use std::{
    collections::VecDeque,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Fixture(pub PathBuf);
impl Fixture {
    // Create a unique private temporary parent; fixture drop removes only this test's directory.
    pub(super) fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "talksphere-identity-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    // Remove only the test-owned tree, never a caller-provided path.
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
// Independent valid fixture staging is never used as production identity material.
pub(super) fn record() -> CredentialRecord {
    let keys = IdentityKeys::generate(&mut ScriptedEntropy(VecDeque::from([
        Ok(vec![1; 32]),
        Ok(vec![2; 32]),
    ])))
    .unwrap();
    CredentialRecord::encode(&keys, limits()).unwrap()
}
// Small finite limits bound all protected filesystem scenarios.
pub(super) fn limits() -> Limits {
    Limits::new(256, 4, 1024, 2048, 4096).unwrap()
}
// Owner-only files load; unauthorized, malformed, oversized and insecure files never yield credentials.
#[test]
fn protected_reads_and_exclusive_locking() {
    let root = Fixture::new();
    let path = root.0.join("store");
    let (store, admin) = FileStore::open(&path, limits()).unwrap();
    assert!(matches!(
        FileStore::open(&path, limits()),
        Err(crate::modules::identity::error::IdentityError::Conflict)
    ));
    let outsider = Authority::new();
    assert!(store.read(&outsider.administration(4096).unwrap()).is_err());
    assert!(store.read(&admin).is_err());
    let file = path.join("identity.bin");
    fs::write(&file, record().protected_bytes()).unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    store.read(&admin).unwrap().decode().unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(store.read(&admin).is_err());
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(&file, [0; 4097]).unwrap();
    assert!(store.read(&admin).is_err());
    fs::write(&file, [0; 4]).unwrap();
    assert!(store.read(&admin).is_err());
    fs::remove_file(&file).unwrap();
    symlink("writer.lock", &file).unwrap();
    assert!(store.read(&admin).is_err());
    fs::remove_file(&file).unwrap();
    fs::hard_link(path.join("writer.lock"), &file).unwrap();
    assert!(store.read(&admin).is_err());
    drop(store);
}
// Traversal, symlinked directories and insecure owner modes fail before protected reads.
#[test]
fn paths_and_directory_protection_are_checked() {
    let root = Fixture::new();
    let path = root.0.join("store");
    let (store, _) = FileStore::open(&path, limits()).unwrap();
    drop(store);
    assert!(FileStore::open(&path.join(".."), limits()).is_err());
    symlink(&path, root.0.join("alias")).unwrap();
    assert!(FileStore::open(&root.0.join("alias"), limits()).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(FileStore::open(&path, limits()).is_err());
    assert!(FileStore::open(std::path::Path::new(""), limits()).is_err());
    assert!(FileStore::open(&root.0.join("absent/child"), limits()).is_err());
}
