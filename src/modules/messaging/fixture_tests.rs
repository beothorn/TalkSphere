//! Cross-checks of the independently generated vectors against TalkSphere's own primitives.
//!
//! `fixtures/vectors.json` is produced by `fixtures/generate.py` with PyNaCl and `hashlib`; these tests
//! use its literal bytes and confirm that Rust derives the same identities and verifies the Python
//! signatures. Rust-computed expectations are never the source of truth.
use crate::modules::{
    cryptography::{
        credential,
        primitives::{hash, signature},
    },
    messaging::{ids::Signature, ports::Signer, test_support::TestIdentity},
};
use serde_json::Value;

// The parsed vector file.
fn vectors() -> Value {
    serde_json::from_str(include_str!("fixtures/vectors.json")).unwrap()
}

// Decode a lowercase hex member.
pub(crate) fn bytes(value: &Value) -> Vec<u8> {
    let text = value.as_str().unwrap();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

// Rust derives the same public keys and identity IDs as the independent generator for all three identities.
#[test]
fn identities_match_the_independent_derivation() {
    let v = vectors();
    for (name, seed) in [("sender", 1u8), ("provider", 2), ("recipient", 3)] {
        let identity = TestIdentity::new(seed);
        let expected = &v["identities"][name];
        assert_eq!(
            identity.key().to_vec(),
            bytes(&expected["publicKey"]),
            "{name}"
        );
        assert_eq!(
            identity.identity().0.to_vec(),
            bytes(&expected["id"]),
            "{name}"
        );
        assert_eq!(
            credential::identity_id(&identity.key()).unwrap().to_vec(),
            bytes(&expected["id"])
        );
    }
}

// The literal `abc` hash, the signing bytes length and each Python signature verify under the Rust verifier,
// and Rust's deterministic Ed25519 signatures equal the Python ones byte for byte.
#[test]
fn signatures_agree_in_both_directions() {
    let v = vectors();
    assert_eq!(
        hash::sha256(b"abc", 3).unwrap().to_vec(),
        bytes(&v["message"]["payloadHash"])
    );
    // 16 domain + 32 + 32 + 32 + 8 + 1 + 8 + 32 payload hash + 3 payload bytes.
    assert_eq!(bytes(&v["message"]["signingBytes"]).len(), 164);
    for (signer, message, signature_hex) in [
        (
            1u8,
            &v["message"]["signingBytes"],
            &v["message"]["senderSignature"],
        ),
        (
            3,
            &v["recovery"]["requestBytes"],
            &v["recovery"]["recipientSignature"],
        ),
        (
            2,
            &v["recovery"]["receiptBytes"],
            &v["recovery"]["providerSignature"],
        ),
    ] {
        let identity = TestIdentity::new(signer);
        let (message, expected) = (bytes(message), bytes(signature_hex));
        assert!(signature::verify(&identity.key(), &message, &expected, 1024).is_ok());
        assert_eq!(
            identity.sign(&message).unwrap(),
            Signature(expected.try_into().unwrap())
        );
    }
}
