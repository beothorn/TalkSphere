//! CR-10/CR-11 demonstrate construction boundaries without implementing Messaging or Storage.

use super::{decrypt::decrypt, encrypt::encrypt, object};
use crate::modules::cryptography::{
    api::{DecryptRequest, EncryptRequest},
    config::ResourceLimits,
    primitives::{aead, hash, signature},
    secrets::{Content, Persistent, Secret, Signing},
    test_support::{ScriptedEntropy, envelope, hex, recipient},
};
use std::collections::VecDeque;
use zeroize::Zeroizing;

// Read independently generated literals, not output from the production encoder under test.
fn messaging(label: &str) -> Vec<u8> {
    let text = include_str!("../../test_support/fixtures/messaging.md");
    hex(text
        .split(&format!("\n{label}:\n"))
        .nth(1)
        .unwrap()
        .lines()
        .next()
        .unwrap())
}

// The independently valid Messaging ciphertext authenticates, but its payload is not a generic object.
#[test]
fn messaging_envelope_is_not_the_generic_format() {
    let key = Secret::<Content>::new(Zeroizing::new(messaging("derived key").try_into().unwrap()));
    assert_eq!(
        aead::open(
            &key,
            &messaging("nonce"),
            &messaging("sealed"),
            &messaging("metadata"),
            1024
        )
        .unwrap(),
        messaging("plaintext")
    );
    let payload = messaging("payload");
    assert_eq!(payload.len(), 89 + messaging("sealed").len());
    assert!(object::parse(&payload, ResourceLimits::new(1024, 1172).unwrap()).is_err());
}

// A test-only application manifest binds each ciphertext's position, byte length and digest.
fn manifest_matches(objects: &[Vec<u8>], manifest: &[(usize, usize, [u8; 32])]) -> bool {
    objects.len() == manifest.len()
        && objects.iter().zip(manifest).enumerate().all(
            |(position, (bytes, (expected_position, length, digest)))| {
                position == *expected_position
                    && bytes.len() == *length
                    && hash::sha256(bytes, 1172).unwrap() == *digest
            },
        )
}

// Opaque storage needs no keys; independently valid chunks do not prove sequence or completeness.
#[test]
fn opaque_retrieval_and_application_manifest_preserve_separate_guarantees() {
    let recipient = recipient();
    let limits = ResourceLimits::new(1024, 1172).unwrap();
    let mut entropy = ScriptedEntropy(VecDeque::from([
        Ok(vec![1; 32]),
        Ok(vec![3; 24]),
        Ok(vec![2; 32]),
        Ok(vec![4; 24]),
    ]));
    let first = encrypt(
        EncryptRequest {
            plaintext: b"first",
            recipient: &recipient,
            limits,
        },
        &mut entropy,
    )
    .unwrap();
    let second = encrypt(
        EncryptRequest {
            plaintext: b"second",
            recipient: &recipient,
            limits,
        },
        &mut entropy,
    )
    .unwrap();
    let mut opaque_store = vec![first, second];
    let manifest: Vec<_> = opaque_store
        .iter()
        .enumerate()
        .map(|(position, bytes)| (position, bytes.len(), hash::sha256(bytes, 1172).unwrap()))
        .collect();
    // This signed encoding is test application policy, never a proposed CR2SE wire format.
    let mut manifest_bytes = Vec::new();
    for (position, length, digest) in &manifest {
        manifest_bytes.extend_from_slice(&(*position as u64).to_be_bytes());
        manifest_bytes.extend_from_slice(&(*length as u64).to_be_bytes());
        manifest_bytes.extend_from_slice(digest);
    }
    let signing = Secret::<Signing>::new(Zeroizing::new(
        envelope("recipient Ed25519 seed").try_into().unwrap(),
    ));
    let proof = signature::sign(&signing, &manifest_bytes, 1024).unwrap();
    signature::verify(&recipient.signing_public_key, &manifest_bytes, &proof, 1024).unwrap();
    let mut altered_manifest = manifest_bytes.clone();
    altered_manifest[0] ^= 1;
    assert!(
        signature::verify(
            &recipient.signing_public_key,
            &altered_manifest,
            &proof,
            1024
        )
        .is_err()
    );
    assert!(manifest_matches(&opaque_store, &manifest));
    opaque_store.swap(0, 1);
    assert!(!manifest_matches(&opaque_store, &manifest));
    let key = Secret::<Persistent>::new(Zeroizing::new(
        envelope("recipient raw persistent X25519 private-key input")
            .try_into()
            .unwrap(),
    ));
    for (bytes, expected) in opaque_store.iter().zip([b"second".as_slice(), b"first"]) {
        let plaintext = decrypt(DecryptRequest {
            object: bytes,
            identity_id: recipient.identity_id,
            encryption_public_key: recipient.encryption_public_key,
            private_key: &key,
            limits,
        })
        .unwrap();
        assert_eq!(plaintext, expected);
    }
    opaque_store.pop();
    assert!(!manifest_matches(&opaque_store, &manifest));
}
