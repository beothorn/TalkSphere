//! Deterministic protected lifecycle scenarios; private fixture values never reach production configuration.
use super::*;
use crate::modules::cryptography::{
    error::CryptoError,
    test_support::{ScriptedEntropy, observe_releases},
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, atomic::Ordering},
};
#[derive(Default)]
struct Store {
    durable: Arc<Mutex<Option<zeroize::Zeroizing<Vec<u8>>>>>,
    failure: Option<IdentityError>,
}
impl KeyStore for Store {
    type Administration = bool;
    type Record = CredentialRecord;
    // Test-only authority checks run before resource or existence-sensitive effects.
    fn authorize(&self, admin: &bool, bytes: usize) -> Result<(), IdentityError> {
        if !admin {
            return Err(IdentityError::AccessDenied);
        }
        if bytes > 4096 {
            return Err(IdentityError::ResourceLimit);
        }
        Ok(())
    }
    // Publish only a complete record; failure cannot silently install one of the private keys.
    fn install(&mut self, admin: &bool, record: CredentialRecord) -> Result<(), IdentityError> {
        self.authorize(admin, record.len())?;
        if let Some(error) = self.failure {
            return Err(error);
        }
        let mut state = self.durable.lock().unwrap();
        if state.is_some() {
            return Err(IdentityError::Conflict);
        }
        *state = Some(record.protected_bytes().to_vec().into());
        Ok(())
    }
    // Simulate protected restart state without generating fresh material.
    fn open(&self, admin: &bool) -> Result<CredentialRecord, IdentityError> {
        self.authorize(admin, RECORD_BYTES)?;
        let bytes = self
            .durable
            .lock()
            .unwrap()
            .clone()
            .ok_or(IdentityError::Unavailable)?;
        CredentialRecord::from_protected_bytes(bytes, limits())
    }
}
// Finite independent local limits allow complete records and bounded envelopes.
fn limits() -> Limits {
    Limits::new(256, 8, 4096, 8192, 4096).unwrap()
}
// Script distinct private inputs; consuming exactly these two requests is part of the scenario.
fn entropy() -> ScriptedEntropy {
    ScriptedEntropy(VecDeque::from([Ok(vec![1; 32]), Ok(vec![2; 32])]))
}
// Creation is denied before entropy and publishes a complete public identity only after durable installation.
#[test]
fn creation_checks_authority_and_commits_complete_association() {
    let store = Store::default();
    let durable = store.durable.clone();
    let mut provider = Identity::new(store, entropy(), limits(), 1).unwrap();
    assert!(matches!(
        provider.create(&false),
        Err(IdentityError::AccessDenied)
    ));
    assert_eq!(provider.entropy.0.len(), 2);
    assert!(durable.lock().unwrap().is_none());
    let (handle, public) = provider.create(&true).unwrap();
    assert!(provider.entropy.0.is_empty());
    let record = provider.store.open(&true).unwrap();
    assert_eq!(
        record.decode().unwrap().credential().unwrap().identity_id,
        public.identity.id().0
    );
    let read = provider.grant::<ReadPublic>(&true, &handle, 1).unwrap();
    assert_eq!(provider.public(&handle, &read).unwrap(), public);
    assert!(provider.grant::<ReadPublic>(&false, &handle, 1).is_err());
    let other = Identity::new(Store::default(), entropy(), limits(), 1).unwrap();
    assert!(other.grant::<ReadPublic>(&true, &handle, 1).is_err());
    assert!(other.public(&handle, &read).is_err());
    provider.authority.revoke();
    assert!(provider.public(&handle, &read).is_err());
    assert!(Identity::new(Store::default(), entropy(), limits(), 2).is_err());
    assert!(
        Identity::new(
            Store::default(),
            entropy(),
            Limits::new(58, 1, 1, 148, 1).unwrap(),
            1
        )
        .is_err()
    );
}
// Entropy and installation failures retain no usable handle, release acquired keys and never retry.
#[test]
fn creation_failures_erase_staging_without_automatic_replacement() {
    for values in [
        VecDeque::from([Err(CryptoError::Unavailable)]),
        VecDeque::from([Ok(vec![1; 32]), Err(CryptoError::Unavailable)]),
    ] {
        let expected = values.len() - 1;
        let (result, released) = observe_releases(|| {
            let mut provider =
                Identity::new(Store::default(), ScriptedEntropy(values), limits(), 1).unwrap();
            provider.create(&true)
        });
        assert!(matches!(result, Err(IdentityError::Unavailable)));
        assert_eq!(released.load(Ordering::SeqCst), expected);
    }
    for error in [
        IdentityError::Unavailable,
        IdentityError::Conflict,
        IdentityError::OutcomeUnknown,
    ] {
        let store = Store {
            failure: Some(error),
            ..Store::default()
        };
        let durable = store.durable.clone();
        let (result, released) = observe_releases(|| {
            let mut provider = Identity::new(store, entropy(), limits(), 1).unwrap();
            let result = provider.create(&true);
            assert!(provider.entropy.0.is_empty());
            result
        });
        assert!(matches!(result,Err(e) if e==error));
        assert_eq!(released.load(Ordering::SeqCst), 2);
        assert!(durable.lock().unwrap().is_none());
    }
}

// Restart restores the exact credential, while denied, partial and mismatched records never consume entropy.
#[test]
fn loading_validates_without_replacement() {
    let mut provider = Identity::new(Store::default(), entropy(), limits(), 1).unwrap();
    assert!(matches!(
        provider.load(&true),
        Err(IdentityError::Unavailable)
    ));
    assert!(matches!(
        provider.load(&false),
        Err(IdentityError::AccessDenied)
    ));
    assert_eq!(provider.entropy.0.len(), 2);
    let (_, expected) = provider.create(&true).unwrap();
    let original = provider.store.durable.lock().unwrap().clone().unwrap();
    let mut restored = Identity::new(provider.store, entropy(), limits(), 1).unwrap();
    let (handle, public) = restored.load(&true).unwrap();
    assert_eq!(public, expected);
    let read = restored.grant::<ReadPublic>(&true, &handle, 1).unwrap();
    assert_eq!(restored.public(&handle, &read).unwrap(), expected);
    // Both private authorities reproduce the original complete protected association.
    let encoded = CredentialRecord::encode(&handle.keys, limits()).unwrap();
    assert_eq!(encoded.protected_bytes(), original.as_slice());
    drop(handle);
    for index in [8, 40, 72, 104, 136, 168] {
        let mut corrupted = original.clone();
        corrupted[index] ^= 0x10;
        *restored.store.durable.lock().unwrap() = Some(corrupted.clone());
        let (result, released) = observe_releases(|| restored.load(&true));
        assert!(matches!(result, Err(IdentityError::IdentityMismatch)));
        assert_eq!(released.load(Ordering::SeqCst), 2);
        assert_eq!(
            restored
                .store
                .durable
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .as_slice(),
            corrupted.as_slice()
        );
    }
    *restored.store.durable.lock().unwrap() = Some(original[..100].to_vec().into());
    assert!(matches!(
        restored.load(&true),
        Err(IdentityError::InvalidInput)
    ));
    assert_eq!(restored.entropy.0.len(), 2);
}
