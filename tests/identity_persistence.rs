//! Real Linux Identity lifecycle restart and corrupt-record rejection through public contracts.
#![cfg(target_os = "linux")]
use std::{fs, os::unix::fs::PermissionsExt, process::Command};
use talksphere::modules::{
    cryptography::random::SystemEntropy,
    identity::{api::IdentityProvider, config::Limits, provider::Identity, storage::FileStore},
};

// A fresh process must load the same public association without creating a new identity.
#[test]
fn restart_preserves_identity() {
    let limits = Limits::new(256, 8, 4096, 8192, 4096).unwrap();
    if let Some(path) = std::env::var_os("TALKSPHERE_LOAD_TEST_STORE") {
        let (store, admin) = FileStore::open(std::path::Path::new(&path), limits).unwrap();
        let mut identity = Identity::new(store, SystemEntropy, limits, 1).unwrap();
        let (_, public) = identity.load(&admin).unwrap();
        assert_eq!(
            format!("{public:?}"),
            std::env::var("TALKSPHERE_LOAD_TEST_PUBLIC").unwrap()
        );
        return;
    }
    let path = std::env::temp_dir().join(format!("talksphere-load-{}", std::process::id()));
    fs::create_dir(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    let (store, admin) = FileStore::open(&path, limits).unwrap();
    let mut identity = Identity::new(store, SystemEntropy, limits, 1).unwrap();
    assert!(identity.load(&admin).is_err());
    assert!(!path.join("identity.bin").exists());
    let (handle, public) = identity.create(&admin).unwrap();
    drop(handle);
    drop(identity);
    assert!(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "restart_preserves_identity"])
            .env("TALKSPHERE_LOAD_TEST_STORE", &path)
            .env("TALKSPHERE_LOAD_TEST_PUBLIC", format!("{public:?}"))
            .status()
            .unwrap()
            .success()
    );
    let file = path.join("identity.bin");
    let original = zeroize::Zeroizing::new(fs::read(&file).unwrap());
    for bytes in [original[..100].to_vec(), {
        let mut changed = original.to_vec();
        changed[168] ^= 1;
        changed
    }] {
        let bytes = zeroize::Zeroizing::new(bytes);
        fs::write(&file, &bytes).unwrap();
        let (store, admin) = FileStore::open(&path, limits).unwrap();
        let mut identity = Identity::new(store, SystemEntropy, limits, 1).unwrap();
        assert!(identity.load(&admin).is_err());
        assert_eq!(&*zeroize::Zeroizing::new(fs::read(&file).unwrap()), &*bytes);
    }
    fs::remove_dir_all(path).unwrap();
}
