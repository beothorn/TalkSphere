//! CR-09 observes real secret lifetimes, fresh material and failure before output.

use super::{decrypt::decrypt, encrypt::encrypt, object};
use crate::modules::cryptography::{
    api::{DecryptRequest, EncryptRequest},
    config::ResourceLimits,
    error::CryptoError,
    primitives::keys,
    secrets::{Persistent, Secret},
    test_support::{ScriptedEntropy, envelope, object_entropy, observe_releases, recipient},
};
use std::{collections::VecDeque, sync::atomic::Ordering};
use zeroize::Zeroizing;

// Success drops ephemeral/shared/PRK/content keys; nonce failure still drops the acquired ephemeral key.
#[test]
fn composed_operations_release_every_acquired_secret() {
    let recipient = recipient();
    let limits = ResourceLimits::new(1024, 1172).unwrap();
    for fail_at in [None, Some(0), Some(1)] {
        let mut entropy = object_entropy();
        if let Some(index) = fail_at {
            entropy.0[index] = Err(CryptoError::Unavailable);
        }
        let (result, count) = observe_releases(|| {
            encrypt(
                EncryptRequest {
                    plaintext: b"test",
                    recipient: &recipient,
                    limits,
                },
                &mut entropy,
            )
        });
        assert_eq!(
            count.load(Ordering::SeqCst),
            match fail_at {
                None => 4,
                Some(0) => 0,
                _ => 1,
            }
        );
        assert_eq!(result.is_ok(), fail_at.is_none());
    }
    let private = Secret::<Persistent>::new(Zeroizing::new(
        envelope("recipient raw persistent X25519 private-key input")
            .try_into()
            .unwrap(),
    ));
    let mut object = encrypt(
        EncryptRequest {
            plaintext: b"test",
            recipient: &recipient,
            limits,
        },
        &mut object_entropy(),
    )
    .unwrap();
    for tamper in [false, true] {
        if tamper {
            *object.last_mut().unwrap() ^= 1;
        }
        let (result, count) = observe_releases(|| {
            decrypt(DecryptRequest {
                object: &object,
                identity_id: recipient.identity_id,
                encryption_public_key: recipient.encryption_public_key,
                private_key: &private,
                limits,
            })
        });
        assert_eq!(result.is_ok(), !tamper);
        assert_eq!(count.load(Ordering::SeqCst), 3);
    }
    let (_, count) = observe_releases(|| {
        let abandoned = keys::ephemeral_key(&mut object_entropy()).unwrap();
        drop(abandoned);
    });
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

// Fresh scripted material is consumed for both identical requests; each object decrypts independently.
#[test]
fn repeated_plaintext_uses_new_ephemeral_keys_and_nonces() {
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
            plaintext: b"same",
            recipient: &recipient,
            limits,
        },
        &mut entropy,
    )
    .unwrap();
    let second = encrypt(
        EncryptRequest {
            plaintext: b"same",
            recipient: &recipient,
            limits,
        },
        &mut entropy,
    )
    .unwrap();
    assert!(entropy.0.is_empty());
    assert_ne!(first, second);
    let a = object::parse(&first, limits).unwrap();
    let b = object::parse(&second, limits).unwrap();
    assert_ne!(a.header.ephemeral_key, b.header.ephemeral_key);
    assert_ne!(a.header.nonce, b.header.nonce);
    let private = Secret::<Persistent>::new(Zeroizing::new(
        envelope("recipient raw persistent X25519 private-key input")
            .try_into()
            .unwrap(),
    ));
    for bytes in [&first, &second] {
        assert_eq!(
            decrypt(DecryptRequest {
                object: bytes,
                identity_id: recipient.identity_id,
                encryption_public_key: recipient.encryption_public_key,
                private_key: &private,
                limits
            })
            .unwrap(),
            b"same"
        );
    }
}
