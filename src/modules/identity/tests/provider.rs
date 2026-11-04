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

// Scoped signing matches pinned Transport bytes and rejects changed bytes, mismatched handles and exhausted authority.
#[test]
fn signing_preserves_exact_protocol_bytes_and_local_authority() {
    use crate::modules::cryptography::primitives::signature;
    use crate::modules::identity::{
        authority::Sign,
        test_support::{TRANSPORT, vector},
    };
    let provider = Identity::new(Store::default(), entropy(), limits(), 1).unwrap();
    let keys = IdentityKeys::import(
        vector(TRANSPORT, "initiator Ed25519 seed").into(),
        vec![2; 32].into(),
    )
    .unwrap();
    let (handle, public) = provider.handle(keys).unwrap();
    let mut message = b"CR2SE-NETWORK-AUTH-V1\0".to_vec();
    message.push(1);
    for role in ["initiator", "acceptor"] {
        let hello = vector(TRANSPORT, &format!("{role} HELLO"));
        message.extend_from_slice(&(hello.len() as u32).to_be_bytes());
        message.extend(hello);
    }
    let capability = provider.grant::<Sign>(&true, &handle, 8192).unwrap();
    let signed = provider.sign(&handle, &capability, &message).unwrap();
    assert_eq!(
        signed.as_slice(),
        vector(TRANSPORT, "initiator AUTH payload")
    );
    message[0] ^= 1;
    assert!(
        signature::verify(
            &public.identity.signing_public_key(),
            &message,
            &signed,
            8192
        )
        .is_err()
    );
    let changed = provider.sign(&handle, &capability, &message).unwrap();
    assert_ne!(signed, changed);
    let small = provider.grant::<Sign>(&true, &handle, 1).unwrap();
    assert_eq!(
        provider.sign(&handle, &small, &message),
        Err(IdentityError::ResourceLimit)
    );
    assert_eq!(
        provider.sign(&handle, &capability, &vec![0; 4097]),
        Err(IdentityError::ResourceLimit)
    );
    let wrong = provider
        .authority
        .grant::<Sign>(IdentityId([0; 32]), 8192)
        .unwrap();
    assert_eq!(
        provider.sign(&handle, &wrong, &message),
        Err(IdentityError::AccessDenied)
    );
    let foreign = Identity::new(Store::default(), entropy(), limits(), 1).unwrap();
    assert_eq!(
        foreign.sign(&handle, &capability, &message),
        Err(IdentityError::AccessDenied)
    );
    let other_cap = foreign
        .authority
        .grant::<Sign>(public.identity.id(), 8192)
        .unwrap();
    assert_eq!(
        provider.sign(&handle, &other_cap, &message),
        Err(IdentityError::AccessDenied)
    );
    assert!(provider.sign(&handle, &capability, b"").is_ok());
    provider.authority.revoke();
    assert_eq!(
        provider.sign(&handle, &capability, &message),
        Err(IdentityError::AccessDenied)
    );
}

// Fixed and freshly encrypted objects decrypt only with the intended loaded recipient and authorized format.
#[test]
fn decryption_checks_recipient_authority_format_and_complete_authentication() {
    use crate::modules::{
        cryptography::{
            api::EncryptRequest, config::ResourceLimits, envelope::encrypt, random::SystemEntropy,
        },
        identity::{
            authority::Decrypt,
            operations::EnvelopeFormat,
            test_support::{ENVELOPE, vector},
        },
    };
    let provider = Identity::new(Store::default(), entropy(), limits(), 1).unwrap();
    let keys = IdentityKeys::import(
        vector(ENVELOPE, "recipient Ed25519 seed").into(),
        vector(
            ENVELOPE,
            "recipient raw persistent X25519 private-key input",
        )
        .into(),
    )
    .unwrap();
    let recipient = keys.credential().unwrap();
    let (handle, _) = provider.handle(keys).unwrap();
    let cap = provider.grant::<Decrypt>(&true, &handle, 16384).unwrap();
    let mut object = vector(ENVELOPE, "132-byte header");
    object.extend(vector(ENVELOPE, "sealed ciphertext plus tag (46 bytes)"));
    assert_eq!(
        provider
            .decrypt(&handle, &cap, EnvelopeFormat::GenericV1, &object)
            .unwrap()
            .as_slice(),
        vector(
            ENVELOPE,
            "plaintext ASCII(\"CR2SE identity encryption test\")"
        )
    );
    for index in [0, 4, 36, object.len() - 1] {
        object[index] ^= 1;
        assert!(
            provider
                .decrypt(&handle, &cap, EnvelopeFormat::GenericV1, &object)
                .is_err()
        );
        object[index] ^= 1;
    }
    assert!(
        provider
            .decrypt(
                &handle,
                &cap,
                EnvelopeFormat::GenericV1,
                &object[..object.len() - 16]
            )
            .is_err()
    );
    assert_eq!(
        provider.decrypt(&handle, &cap, EnvelopeFormat::ServiceSpecific, &object),
        Err(IdentityError::Unsupported)
    );
    assert_eq!(
        provider.decrypt(&handle, &cap, EnvelopeFormat::GenericV1, &vec![0; 8193]),
        Err(IdentityError::ResourceLimit)
    );
    let small = provider.grant::<Decrypt>(&true, &handle, 1).unwrap();
    assert_eq!(
        provider.decrypt(&handle, &small, EnvelopeFormat::GenericV1, &object),
        Err(IdentityError::ResourceLimit)
    );
    let wrong = provider
        .authority
        .grant::<Decrypt>(IdentityId([0; 32]), 8192)
        .unwrap();
    assert_eq!(
        provider.decrypt(&handle, &wrong, EnvelopeFormat::GenericV1, &object),
        Err(IdentityError::AccessDenied)
    );
    let other = Identity::new(Store::default(), entropy(), limits(), 1).unwrap();
    assert_eq!(
        other.decrypt(&handle, &cap, EnvelopeFormat::GenericV1, &object),
        Err(IdentityError::AccessDenied)
    );
    let wrong_keys = IdentityKeys::import(vec![9; 32].into(), vec![8; 32].into()).unwrap();
    let (wrong_handle, _) = provider.handle(wrong_keys).unwrap();
    let wrong_cap = provider
        .grant::<Decrypt>(&true, &wrong_handle, 8192)
        .unwrap();
    assert!(
        provider
            .decrypt(
                &wrong_handle,
                &wrong_cap,
                EnvelopeFormat::GenericV1,
                &object
            )
            .is_err()
    );
    let generated = encrypt::encrypt(
        EncryptRequest {
            plaintext: b"application data",
            recipient: &recipient,
            limits: ResourceLimits::new(1024, 8192).unwrap(),
        },
        &mut SystemEntropy,
    )
    .unwrap();
    assert_eq!(
        provider
            .decrypt(&handle, &cap, EnvelopeFormat::GenericV1, &generated)
            .unwrap()
            .as_slice(),
        b"application data"
    );
    provider.authority.revoke();
    assert_eq!(
        provider.decrypt(&handle, &cap, EnvelopeFormat::GenericV1, &object),
        Err(IdentityError::AccessDenied)
    );
}

// Backup requires both scoped source authority and destination administration; failure never modifies the live handle.
#[test]
fn backup_is_complete_authorized_and_preserves_source() {
    use crate::modules::identity::authority::Backup;
    let mut provider = Identity::new(Store::default(), entropy(), limits(), 1).unwrap();
    let (handle, public) = provider.create(&true).unwrap();
    let cap = provider
        .grant::<Backup>(&true, &handle, RECORD_BYTES)
        .unwrap();
    let mut destination = Store::default();
    assert_eq!(
        provider.backup(&handle, &cap, &mut destination, &false),
        Err(IdentityError::AccessDenied)
    );
    let foreign = Authority::new()
        .grant::<Backup>(public.identity.id(), RECORD_BYTES)
        .unwrap();
    assert_eq!(
        provider.backup(&handle, &foreign, &mut destination, &true),
        Err(IdentityError::AccessDenied)
    );
    let small = provider.grant::<Backup>(&true, &handle, 1).unwrap();
    assert_eq!(
        provider.backup(&handle, &small, &mut destination, &true),
        Err(IdentityError::ResourceLimit)
    );
    for error in [IdentityError::Unavailable, IdentityError::OutcomeUnknown] {
        destination.failure = Some(error);
        assert_eq!(
            provider.backup(&handle, &cap, &mut destination, &true),
            Err(error)
        );
        assert!(destination.durable.lock().unwrap().is_none());
    }
    destination.failure = None;
    provider
        .backup(&handle, &cap, &mut destination, &true)
        .unwrap();
    assert_eq!(
        destination.open(&true).unwrap().protected_bytes(),
        provider.store.open(&true).unwrap().protected_bytes()
    );
    assert_eq!(
        provider.backup(&handle, &cap, &mut destination, &true),
        Err(IdentityError::Conflict)
    );
    let read = provider.grant::<ReadPublic>(&true, &handle, 1).unwrap();
    assert_eq!(provider.public(&handle, &read).unwrap(), public);
}

// Import checks both administrative boundaries, validates cached associations and never consumes fresh entropy.
#[test]
fn restore_rejects_denied_corrupt_and_conflicting_imports() {
    let mut original = Identity::new(Store::default(), entropy(), limits(), 1).unwrap();
    let (_, public) = original.create(&true).unwrap();
    let mut target = Identity::new(Store::default(), entropy(), limits(), 1).unwrap();
    let expected = public.identity.id();
    assert!(target.grant_import(&false, expected, RECORD_BYTES).is_err());
    let cap = target.grant_import(&true, expected, RECORD_BYTES).unwrap();
    assert!(matches!(
        target.restore(&false, &cap, expected, &original.store, &true),
        Err(IdentityError::AccessDenied)
    ));
    assert!(matches!(
        target.restore(&true, &cap, expected, &original.store, &false),
        Err(IdentityError::AccessDenied)
    ));
    let wrong = IdentityId([0; 32]);
    assert!(matches!(
        target.restore(&true, &cap, wrong, &original.store, &true),
        Err(IdentityError::AccessDenied)
    ));
    let wrong_cap = target.grant_import(&true, wrong, RECORD_BYTES).unwrap();
    assert!(matches!(
        target.restore(&true, &wrong_cap, wrong, &original.store, &true),
        Err(IdentityError::IdentityMismatch)
    ));
    let bytes = original.store.durable.lock().unwrap().clone().unwrap();
    original.store.durable.lock().unwrap().as_mut().unwrap()[168] ^= 1;
    assert!(matches!(
        target.restore(&true, &cap, expected, &original.store, &true),
        Err(IdentityError::IdentityMismatch)
    ));
    *original.store.durable.lock().unwrap() = Some(bytes);
    let (_, restored) = target
        .restore(&true, &cap, expected, &original.store, &true)
        .unwrap();
    assert_eq!(restored, public);
    assert_eq!(target.entropy.0.len(), 2);
    assert!(matches!(
        target.restore(&true, &cap, expected, &original.store, &true),
        Err(IdentityError::Conflict)
    ));
}

// A signing-only replacement cannot decrypt old objects; changing signing authority cannot preserve an ID.
#[test]
fn signing_only_recovery_cannot_claim_old_decryption_or_inherited_identity() {
    use crate::modules::{
        cryptography::identity_keys::IdentityKeys,
        identity::{
            authority::Decrypt,
            operations::EnvelopeFormat,
            test_support::{ENVELOPE, vector},
        },
    };
    let provider = Identity::new(Store::default(), entropy(), limits(), 1).unwrap();
    let seed = vector(ENVELOPE, "recipient Ed25519 seed");
    let signing_only = IdentityKeys::import(seed.clone().into(), Vec::new().into());
    assert!(signing_only.is_err());
    let keys = IdentityKeys::import(seed.into(), vec![7; 32].into()).unwrap();
    let (handle, public) = provider.handle(keys).unwrap();
    assert_eq!(
        public.identity.id().0.as_slice(),
        vector(ENVELOPE, "recipient CR2SE ID")
    );
    let cap = provider.grant::<Decrypt>(&true, &handle, 8192).unwrap();
    let mut object = vector(ENVELOPE, "132-byte header");
    object.extend(vector(ENVELOPE, "sealed ciphertext plus tag (46 bytes)"));
    assert!(
        provider
            .decrypt(&handle, &cap, EnvelopeFormat::GenericV1, &object)
            .is_err()
    );
    let changed = IdentityKeys::import(
        vec![9; 32].into(),
        vector(
            ENVELOPE,
            "recipient raw persistent X25519 private-key input",
        )
        .into(),
    )
    .unwrap();
    let (_, new_public) = provider.handle(changed).unwrap();
    assert_ne!(public.identity.id(), new_public.identity.id());
}
