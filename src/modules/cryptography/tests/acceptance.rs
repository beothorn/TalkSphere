//! Final acceptance gaps: malformed complete input, key authority and fixed binding domains.

use super::{
    api::DecryptRequest,
    config::ResourceLimits,
    credential,
    envelope::decrypt::decrypt,
    primitives::signature,
    secrets::{Persistent, Secret, Signing},
    test_support::{envelope, observe_releases, recipient},
};
use std::sync::atomic::Ordering;
use zeroize::Zeroizing;

// CR-07 rejects malformed complete inputs at the public boundary before deriving any secret.
#[test]
fn malformed_complete_objects_release_no_plaintext_or_derived_material() {
    let recipient = recipient();
    let key = Secret::<Persistent>::new(Zeroizing::new(
        envelope("recipient raw persistent X25519 private-key input")
            .try_into()
            .unwrap(),
    ));
    let mut valid = envelope("132-byte header");
    valid.extend(envelope("sealed ciphertext plus tag (46 bytes)"));
    let mut cases = Vec::new();
    for selector in 0..4 {
        let mut bytes = valid.clone();
        bytes[selector] = 2;
        cases.push(bytes);
    }
    for length in [0u64, 15, u64::MAX] {
        let mut bytes = valid.clone();
        bytes[124..132].copy_from_slice(&length.to_be_bytes());
        cases.push(bytes);
    }
    cases.push(valid[..131].to_vec());
    cases.push(valid[..valid.len() - 1].to_vec());
    let mut extra = valid.clone();
    extra.push(0);
    cases.push(extra);
    for object in cases {
        let (result, count) = observe_releases(|| {
            decrypt(DecryptRequest {
                object: &object,
                identity_id: recipient.identity_id,
                encryption_public_key: recipient.encryption_public_key,
                private_key: &key,
                limits: ResourceLimits::new(1024, 1172).unwrap(),
            })
        });
        assert!(result.is_err());
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }
}

// Knowing public key bytes cannot create the corresponding signing capability or a valid proof.
#[test]
fn public_bytes_do_not_grant_existing_private_authority() {
    let recipient = recipient();
    let forged = Secret::<Signing>::new(Zeroizing::new(recipient.signing_public_key));
    let signature = signature::sign(&forged, b"canonical authorized operation", 1024).unwrap();
    assert!(
        signature::verify(
            &recipient.signing_public_key,
            b"canonical authorized operation",
            &signature,
            1024
        )
        .is_err()
    );
}

// Every changed binding-message byte, including the domain, invalidates the literal fixed signature.
#[test]
fn every_binding_message_byte_and_signature_width_is_checked() {
    let recipient = recipient();
    let original = credential::binding_bytes(&recipient.encryption_public_key).unwrap();
    for index in 0..original.len() {
        let mut changed = original;
        changed[index] ^= 1;
        assert!(
            signature::verify(
                &recipient.signing_public_key,
                &changed,
                &recipient.binding_signature,
                1024
            )
            .is_err()
        );
    }
    for size in [0, 31, 33] {
        assert!(
            signature::verify(
                &vec![0; size],
                &original,
                &recipient.binding_signature,
                1024
            )
            .is_err()
        );
    }
    for size in [0, 63, 65] {
        assert!(
            signature::verify(
                &recipient.signing_public_key,
                &original,
                &vec![0; size],
                1024
            )
            .is_err()
        );
    }
}
