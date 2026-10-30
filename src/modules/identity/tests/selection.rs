//! Real key pairs exercise single-identity selection and valid competing bindings.
use super::*;
use crate::modules::{cryptography::identity_keys::IdentityKeys, identity::api::PublicIdentity};

// Import independent fixture pairs and derive their complete public credential using real algorithms.
fn public(signing: u8, encryption: u8) -> PublicCredential {
    let keys = IdentityKeys::import(vec![signing; 32].into(), vec![encryption; 32].into()).unwrap();
    let credential = keys.credential().unwrap();
    PublicCredential {
        identity: PublicIdentity {
            id: IdentityId(credential.identity_id),
            signing_public_key: credential.signing_public_key,
        },
        encryption_public_key: credential.encryption_public_key,
        binding_signature: credential.binding_signature,
    }
}

// Zero/multiple identities and both classes of key change cannot transparently become active configuration.
#[test]
fn configuration_rejects_ambiguous_selection_and_rotation() {
    let current = public(1, 2);
    let replica = public(1, 2);
    let competing = public(1, 3);
    let different = public(4, 2);
    assert_eq!(select_configured(&[]), Err(IdentityError::InvalidInput));
    assert_eq!(
        select_configured(&[current, replica]),
        Err(IdentityError::InvalidInput)
    );
    assert_eq!(select_configured(&[current]), Ok(current));
    assert_eq!(classify(&current, &replica), Ok(KeyChange::Unchanged));
    assert_eq!(
        classify(&current, &competing),
        Ok(KeyChange::InconsistentEncryptionKey)
    );
    assert_eq!(classify(&current, &different), Ok(KeyChange::NewIdentity));
    assert_eq!(require_unchanged(&current, &replica), Ok(()));
    for candidate in [competing, different] {
        assert_eq!(
            require_unchanged(&current, &candidate),
            Err(IdentityError::Conflict)
        );
    }
    let mut invalid = current;
    invalid.binding_signature[0] ^= 1;
    assert!(select_configured(&[invalid]).is_err());
    assert!(classify(&invalid, &current).is_err());
    assert!(classify(&current, &invalid).is_err());
}

// A signed competing key is still a conflict, and rejected candidates leave the retained snapshot unchanged.
#[test]
fn recipient_cache_does_not_choose_between_competing_bindings() {
    let current = public(1, 2);
    let mut pins = RecipientPins::new(1).unwrap();
    assert!(RecipientPins::new(0).is_err());
    assert_eq!(pins.get(current.identity.id()), None);
    pins.retain(current).unwrap();
    pins.retain(public(1, 2)).unwrap();
    assert_eq!(pins.retain(public(1, 3)), Err(IdentityError::Conflict));
    assert_eq!(pins.retain(public(4, 2)), Err(IdentityError::ResourceLimit));
    let mut invalid = current;
    invalid.encryption_public_key[0] ^= 1;
    assert!(pins.retain(invalid).is_err());
    assert_eq!(pins.get(current.identity.id()), Some(current));
}
