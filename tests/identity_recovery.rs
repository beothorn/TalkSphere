//! Real protected restore retains signing and old-object decryption across process restart.
#![cfg(target_os = "linux")]
use std::{fs, os::unix::fs::PermissionsExt, process::Command};
use talksphere::modules::{
    cryptography::{
        api::EncryptRequest, config::ResourceLimits, envelope::encrypt, random::SystemEntropy,
    },
    identity::{
        api::IdentityProvider,
        authority::{Backup, Decrypt, Sign},
        config::Limits,
        operations::EnvelopeFormat,
        provider::Identity,
        storage::FileStore,
    },
};

// A restored pair signs identical bytes and decrypts an object created before backup, including in a fresh process.
#[test]
fn protected_restore_survives_restart() {
    let limits = Limits::new(256, 8, 4096, 8192, 4096).unwrap();
    if let Some(root) = std::env::var_os("TALKSPHERE_RECOVERY_ROOT") {
        let root = std::path::PathBuf::from(root);
        // Provider/lease locations come only from the independently restored application catalog.
        assert_eq!(
            fs::read_to_string(root.join("restored-catalog/catalog")).unwrap(),
            "provider=example-provider\ncontact=127.0.0.1:4242\nlease=lease-17\nhash=application-object-digest\nformat=generic-v1\n"
        );
        let (store, admin) = FileStore::open(&root.join("restored"), limits).unwrap();
        let mut provider = Identity::new(store, SystemEntropy, limits, 1).unwrap();
        let (handle, _) = provider.load(&admin).unwrap();
        let sign = provider.grant::<Sign>(&admin, &handle, 4096).unwrap();
        assert_eq!(
            format!(
                "{:?}",
                provider.sign(&handle, &sign, b"recovery proof").unwrap()
            ),
            std::env::var("TALKSPHERE_RECOVERY_SIGNATURE").unwrap()
        );
        let decrypt = provider.grant::<Decrypt>(&admin, &handle, 8192).unwrap();
        assert_eq!(
            provider
                .decrypt(
                    &handle,
                    &decrypt,
                    EnvelopeFormat::GenericV1,
                    &fs::read(root.join("object")).unwrap()
                )
                .unwrap()
                .as_slice(),
            b"persistent object before backup"
        );
        return;
    }
    let root = std::env::temp_dir().join(format!("talksphere-recovery-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let (store, admin) = FileStore::open(&root.join("source"), limits).unwrap();
    let mut provider = Identity::new(store, SystemEntropy, limits, 1).unwrap();
    let (handle, public) = provider.create(&admin).unwrap();
    let sign = provider.grant::<Sign>(&admin, &handle, 4096).unwrap();
    let signature = provider.sign(&handle, &sign, b"recovery proof").unwrap();
    let recipient = talksphere::modules::cryptography::api::RecipientCredential {
        identity_id: public.identity.id().0,
        signing_public_key: public.identity.signing_public_key(),
        encryption_public_key: public.encryption_public_key,
        binding_signature: public.binding_signature,
    };
    let object = encrypt::encrypt(
        EncryptRequest {
            plaintext: b"persistent object before backup",
            recipient: &recipient,
            limits: ResourceLimits::new(4096, 8192).unwrap(),
        },
        &mut SystemEntropy,
    )
    .unwrap();
    fs::write(root.join("object"), object).unwrap();
    // The catalog is an application fixture, stored and restored separately from either key store.
    fs::create_dir(root.join("catalog-backup")).unwrap();
    fs::write(root.join("catalog-backup/catalog"),
        "provider=example-provider\ncontact=127.0.0.1:4242\nlease=lease-17\nhash=application-object-digest\nformat=generic-v1\n").unwrap();
    let (mut backup, backup_admin) = FileStore::open(&root.join("backup"), limits).unwrap();
    let cap = provider.grant::<Backup>(&admin, &handle, 4096).unwrap();
    provider
        .backup(&handle, &cap, &mut backup, &backup_admin)
        .unwrap();
    let (store, admin) = FileStore::open(&root.join("restored"), limits).unwrap();
    let mut restored = Identity::new(store, SystemEntropy, limits, 1).unwrap();
    let import = restored
        .grant_import(&admin, public.identity.id(), 4096)
        .unwrap();
    let (restored_handle, restored_public) = restored
        .restore(
            &admin,
            &import,
            public.identity.id(),
            &backup,
            &backup_admin,
        )
        .unwrap();
    assert_eq!(public, restored_public);
    // Keys recover cryptographic authority but supply no invented application catalog.
    assert!(!root.join("restored/catalog").exists());
    assert!(!root.join("restored-catalog").exists());
    fs::create_dir(root.join("restored-catalog")).unwrap();
    fs::copy(
        root.join("catalog-backup/catalog"),
        root.join("restored-catalog/catalog"),
    )
    .unwrap();
    drop(restored_handle);
    drop(restored);
    assert!(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "protected_restore_survives_restart"])
            .env("TALKSPHERE_RECOVERY_ROOT", &root)
            .env("TALKSPHERE_RECOVERY_SIGNATURE", format!("{signature:?}"))
            .status()
            .unwrap()
            .success()
    );
    fs::remove_dir_all(root).unwrap();
}
