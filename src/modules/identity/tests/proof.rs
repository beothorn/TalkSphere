//! Proof scenarios distinguish ID relations from exact role-bound possession evidence.
use super::*;
use crate::modules::{
    cryptography::provider::Provider,
    identity::test_support::{CryptoCall, ScriptedCrypto, TRANSPORT, vector},
};
use std::{cell::RefCell, collections::VecDeque};

// Keep proof construction explicit while individual scenarios substitute untrusted fields.
fn proof<'a>(id: &'a [u8], key: &'a [u8], message: &'a [u8], signature: &'a [u8]) -> Proof<'a> {
    Proof {
        version: 1,
        algorithm: 1,
        claimed_id: id,
        public_key: key,
        message,
        signature,
    }
}

// Fixed AUTH proof succeeds only for its exact ID, key, domain, role and transcript.
#[test]
fn pinned_proof_requires_exact_identity_and_protocol_bytes() {
    let crypto = Provider::system(4096).unwrap();
    let id = vector(TRANSPORT, "initiator CR2SE ID");
    let key = vector(TRANSPORT, "initiator Ed25519 public key");
    let signature = vector(TRANSPORT, "initiator AUTH payload");
    let mut message = b"CR2SE-NETWORK-AUTH-V1\0".to_vec();
    message.push(1);
    for role in ["initiator", "acceptor"] {
        let hello = vector(TRANSPORT, &format!("{role} HELLO"));
        message.extend_from_slice(&(hello.len() as u32).to_be_bytes());
        message.extend(hello);
    }
    assert_eq!(
        verify(&crypto, proof(&id, &key, &message, &signature), 4096)
            .unwrap()
            .id()
            .0
            .as_slice(),
        id
    );
    // Repeating a proof is deliberately not replay detection; only the consuming protocol has that state.
    assert!(verify(&crypto, proof(&id, &key, &message, &signature), 4096).is_ok());
    for index in [0, b"CR2SE-NETWORK-AUTH-V1\0".len(), message.len() - 1] {
        message[index] ^= 1;
        assert_eq!(
            verify(&crypto, proof(&id, &key, &message, &signature), 4096),
            Err(IdentityError::InvalidProof)
        );
        message[index] ^= 1;
    }
    assert_eq!(
        verify(&crypto, proof(&[0; 32], &key, &message, &signature), 4096),
        Err(IdentityError::IdentityMismatch)
    );
    let other = vector(TRANSPORT, "acceptor Ed25519 public key");
    assert!(verify(&crypto, proof(&id, &other, &message, &signature), 4096).is_err());
    for length in [0, 63, 65] {
        assert_eq!(
            verify(&crypto, proof(&id, &key, &message, &vec![0; length]), 4096),
            Err(IdentityError::InvalidInput)
        );
    }
    assert_eq!(
        verify(&crypto, proof(&id, &key, &message, &signature), 1),
        Err(IdentityError::ResourceLimit)
    );
    let mut unsupported = proof(&id, &key, &message, &signature);
    unsupported.version = 2;
    assert_eq!(
        verify(&crypto, unsupported, 4096),
        Err(IdentityError::Unsupported)
    );
    let mut weak = [0; 32];
    weak[0] = 1;
    let weak_id = crate::modules::cryptography::credential::identity_id(&weak).unwrap();
    assert!(verify(&crypto, proof(&weak_id, &weak, b"", &[0; 64]), 4096).is_err());
}

// Verify failures are preserved after successful relation validation; hash/key failures stop before proof work.
#[test]
fn proof_dependency_failures_are_preserved_in_order() {
    let key = vec![2; 32];
    let claimed = vec![3; 32];
    let signature = vec![4; 64];
    let mut hash_input = vec![1, 1];
    hash_input.extend(&key);
    for failure_index in 0..3 {
        let mut calls = VecDeque::from([
            CryptoCall {
                operation: "hash",
                inputs: vec![hash_input.clone()],
                result: Ok(claimed.clone()),
            },
            CryptoCall {
                operation: "key",
                inputs: vec![key.clone()],
                result: Ok(vec![]),
            },
            CryptoCall {
                operation: "verify",
                inputs: vec![key.clone(), b"exact".to_vec(), signature.clone()],
                result: Ok(vec![]),
            },
        ]);
        calls[failure_index].result = Err(IdentityError::Unavailable);
        calls.truncate(failure_index + 1);
        let crypto = ScriptedCrypto(RefCell::new(calls));
        assert_eq!(
            verify(&crypto, proof(&claimed, &key, b"exact", &signature), 100),
            Err(IdentityError::Unavailable)
        );
        assert!(crypto.0.borrow().is_empty());
    }
}
