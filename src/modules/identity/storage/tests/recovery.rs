//! Real protected backup destinations exercise atomic failure cuts and complete pair preservation.
use super::{
    FileStore,
    atomic::Cut,
    tests::{Fixture, limits},
};
use crate::modules::{
    cryptography::random::SystemEntropy,
    identity::{
        api::IdentityProvider, authority::Backup, error::IdentityError, provider::Identity,
    },
};

// A protected backup is absent before installation or contains both valid pairs after an uncertain commit.
#[test]
fn backups_obey_protected_destination_durability() {
    let root = Fixture::new();
    let (store, admin) = FileStore::open(&root.0.join("source"), limits()).unwrap();
    let mut provider = Identity::new(store, SystemEntropy, limits(), 1).unwrap();
    let (handle, public) = provider.create(&admin).unwrap();
    let capability = provider.grant::<Backup>(&admin, &handle, 4096).unwrap();
    for (index, cut) in [Cut::Created, Cut::Written, Cut::Synced, Cut::Installed]
        .into_iter()
        .enumerate()
    {
        let path = root.0.join(format!("backup-{index}"));
        let (mut destination, authority) = FileStore::open(&path, limits()).unwrap();
        destination.fault = Some(cut);
        assert_eq!(
            provider.backup(&handle, &capability, &mut destination, &authority),
            Err(if cut == Cut::Installed {
                IdentityError::OutcomeUnknown
            } else {
                IdentityError::Unavailable
            })
        );
        drop(destination);
        let (destination, authority) = FileStore::open(&path, limits()).unwrap();
        assert!(!path.join("identity.pending").exists());
        if cut == Cut::Installed {
            let record = destination.reconcile(&authority).unwrap();
            let restored = record.decode().unwrap().credential().unwrap();
            assert_eq!(restored.identity_id, public.identity.id().0);
            assert_eq!(restored.encryption_public_key, public.encryption_public_key);
            assert_eq!(restored.binding_signature, public.binding_signature);
        } else {
            assert!(destination.read(&authority).is_err());
        }
    }
}
