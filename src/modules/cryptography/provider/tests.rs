//! Real public contract composition, without persistent Identity storage.
use super::*;
use crate::modules::cryptography::{
    credential,
    test_support::{ScriptedEntropy, envelope, object_entropy, recipient},
};
use std::collections::VecDeque;
use zeroize::Zeroizing;

// All declared primitive capabilities are usable through the real provider and preserve exact values.
#[test]
fn public_primitive_contracts_and_configuration() {
    assert!(Provider::system(147).is_err());
    let system = Provider::system(1024).unwrap();
    assert!(system.sha256(b"abc").is_ok());
    let mut entropy = ScriptedEntropy(VecDeque::from([
        Ok(envelope("recipient Ed25519 seed")),
        Ok(envelope(
            "recipient raw persistent X25519 private-key input",
        )),
        Ok(envelope("raw ephemeral X25519 private-key input")),
    ]));
    entropy.0.push_back(Ok(vec![3; 32]));
    let mut provider = Provider::new(entropy, 1024).unwrap();
    let signing = provider.generate_signing_key().unwrap();
    let persistent = provider.generate_persistent_key().unwrap();
    let ephemeral = provider.generate_ephemeral_key().unwrap();
    let signature = provider.sign(&signing.private_key, b"exact").unwrap();
    provider
        .verify(&signing.public_key, b"exact", &signature)
        .unwrap();
    assert!(
        provider
            .verify(&signing.public_key, b"changed", &signature)
            .is_err()
    );
    let a = provider
        .agree_persistent(&persistent.private_key, &ephemeral.public_key)
        .unwrap();
    let b = provider
        .agree_ephemeral(ephemeral.private_key, &persistent.public_key)
        .unwrap();
    assert_eq!(a.bytes, b.bytes);
    assert_eq!(provider.hello_nonce().unwrap(), [3; 32]);
    assert_eq!(provider.hello_nonce(), Err(CryptoError::Unavailable));
    assert_eq!(
        credential::identity_id(&signing.public_key).unwrap(),
        recipient().identity_id
    );
}

// Envelope calls intersect caller budgets with provider policy and return complete authenticated results.
#[test]
fn public_envelope_contracts_and_provider_ceiling() {
    let recipient = recipient();
    let limits = ResourceLimits::new(1024, 1172).unwrap();
    let mut provider = Provider::new(object_entropy(), 1024).unwrap();
    let object = provider
        .encrypt(api::EncryptRequest {
            plaintext: b"hello",
            recipient: &recipient,
            limits,
        })
        .unwrap();
    let key = Secret::new(Zeroizing::new(
        envelope("recipient raw persistent X25519 private-key input")
            .try_into()
            .unwrap(),
    ));
    assert_eq!(
        provider
            .decrypt(api::DecryptRequest {
                object: &object,
                identity_id: recipient.identity_id,
                encryption_public_key: recipient.encryption_public_key,
                private_key: &key,
                limits
            })
            .unwrap(),
        b"hello"
    );
    let mut small = Provider::new(object_entropy(), 148).unwrap();
    assert_eq!(
        small.encrypt(api::EncryptRequest {
            plaintext: b"x",
            recipient: &recipient,
            limits
        }),
        Err(CryptoError::ResourceLimit)
    );
    assert!(
        small
            .decrypt(api::DecryptRequest {
                object: &object,
                identity_id: recipient.identity_id,
                encryption_public_key: recipient.encryption_public_key,
                private_key: &key,
                limits
            })
            .is_err()
    );
}
