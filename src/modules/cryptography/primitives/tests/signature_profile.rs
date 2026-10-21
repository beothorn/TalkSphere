//! Updated CR2SE negative vectors and mixed-order equation regressions.
use super::*;
use crate::modules::cryptography::test_support::{hex, transport};
use curve25519_dalek::{
    constants::{ED25519_BASEPOINT_POINT, EIGHT_TORSION},
    scalar::Scalar,
};
use sha2::{Digest, Sha512};

// Every published negative row rejects both alone and when collected with valid AUTH evidence.
#[test]
fn updated_negative_vectors_and_individual_collection_decisions() {
    let key = transport("initiator Ed25519 public key");
    let signature = transport("initiator AUTH payload");
    let mut message = b"CR2SE-NETWORK-AUTH-V1\0".to_vec();
    message.push(1);
    for role in ["initiator", "acceptor"] {
        let hello = transport(&format!("{role} HELLO"));
        message.extend_from_slice(&(hello.len() as u32).to_be_bytes());
        message.extend(hello);
    }
    let mut invalid = Vec::new();
    let identity = hex("0100000000000000000000000000000000000000000000000000000000000000");
    for text in [
        "0100000000000000000000000000000000000000000000000000000000000000",
        "ecffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
        "edffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
        "0100000000000000000000000000000000000000000000000000000000000080",
    ] {
        let point = hex(text);
        invalid.push((point.clone(), signature.clone(), message.clone()));
        let mut sig = signature.clone();
        sig[..32].copy_from_slice(&point);
        invalid.push((key.clone(), sig, message.clone()));
        assert!(super::super::verification_key::VerificationKey::parse(&point).is_err());
    }
    let order = hex("edd3f55c1a631258d69cf7a2def9de1400000000000000000000000000000010");
    let mut scalar_boundary = signature.clone();
    scalar_boundary[32..].copy_from_slice(&order);
    invalid.push((key.clone(), scalar_boundary, message.clone()));
    let mut malleable = signature.clone();
    let mut carry = 0u16;
    for i in 0..32 {
        let sum = u16::from(signature[32 + i]) + u16::from(order[i]) + carry;
        malleable[32 + i] = sum as u8;
        carry = sum >> 8;
    }
    invalid.push((key.clone(), malleable, message.clone()));
    for text in [b"first".as_slice(), b"different"] {
        let mut sig = identity.clone();
        sig.extend([0; 32]);
        invalid.push((identity.clone(), sig, text.to_vec()));
    }
    assert!(verify(&key, &message, &signature, usize::MAX).is_ok());
    for (a, s, m) in invalid {
        let decisions = [
            verify(&key, &message, &signature, usize::MAX).is_ok(),
            verify(&a, &m, &s, usize::MAX).is_ok(),
        ];
        assert_eq!(decisions, [true, false]);
        assert!(!decisions.into_iter().all(|valid| valid));
    }
}

// Canonical mixed-order keys follow the uncofactored equation, not prime-subgroup policy or cofactor clearing.
#[test]
fn mixed_order_equation_accepts_only_exact_equality() {
    let key = (ED25519_BASEPOINT_POINT + EIGHT_TORSION[1])
        .compress()
        .to_bytes();
    let r = ED25519_BASEPOINT_POINT.compress().to_bytes();
    let mut accepted = false;
    let mut rejected = false;
    for n in 0u16..256 {
        let message = n.to_le_bytes();
        let mut hash = Sha512::new();
        hash.update(r);
        hash.update(key);
        hash.update(message);
        let wide: [u8; 64] = hash.finalize().into();
        let k = Scalar::from_bytes_mod_order_wide(&wide);
        let s = Scalar::ONE + k;
        let mut signature = [0; 64];
        signature[..32].copy_from_slice(&r);
        signature[32..].copy_from_slice(&s.to_bytes());
        let exact = (k * EIGHT_TORSION[1]) == curve25519_dalek::edwards::EdwardsPoint::default();
        assert_eq!(verify(&key, &message, &signature, 2).is_ok(), exact);
        accepted |= exact;
        rejected |= !exact;
        if accepted && rejected {
            break;
        }
    }
    assert!(accepted && rejected);
}
