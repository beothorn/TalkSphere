//! Shape cases from TR-01/TR-04, independent of cryptographic verification.
use super::*;
use crate::modules::transport::test_support::vectors;

// The fixed HELLO payloads must expose the upstream keys/nonces and re-encode exactly.
#[test]
fn hello_fields_match_upstream_bytes_for_both_roles() {
    for role in ["initiator", "acceptor"] {
        let bytes = vectors::bytes(&format!("{role} HELLO"), 0);
        let hello = Hello::decode(&bytes).unwrap();
        assert_eq!(
            hello.identity_public_key,
            vectors::array::<32>(&format!("{role} Ed25519 public key"))
        );
        assert_eq!(
            hello.ephemeral_public_key,
            vectors::array::<32>(&format!("{role} ephemeral X25519 public key"))
        );
        assert_eq!(
            hello.nonce,
            vectors::array::<32>(&format!("{role} HELLO nonce"))
        );
        assert_eq!(hello.encode().as_slice(), bytes);
    }
}
// Every selector value except one fails; no unknown algorithm can silently use a default.
#[test]
fn selectors_and_lengths_are_strict() {
    let baseline = vectors::bytes("initiator HELLO", 0);
    for offset in [0, 1, 2, 35] {
        for value in 0..=u8::MAX {
            if value == 1 {
                continue;
            }
            let mut bytes = baseline.clone();
            bytes[offset] = value;
            assert_eq!(
                Hello::decode(&bytes).err(),
                Some(HandshakePayloadError::Selector { offset, value })
            );
        }
    }
    for length in [0, 99, 101, 65536] {
        assert_eq!(
            Hello::decode(&vec![1; length]).err(),
            Some(HandshakePayloadError::Length {
                expected: 100,
                actual: length
            })
        );
    }
}
// AUTH validates width only; even a zero signature must reach the verifier instead of proving success.
#[test]
fn auth_preserves_fixed_signatures_without_authenticating() {
    for role in ["initiator", "acceptor"] {
        let bytes = vectors::bytes(&format!("{role} AUTH payload"), 0);
        assert_eq!(Auth::decode(&bytes).unwrap().encode().as_slice(), bytes);
    }
    assert_eq!(Auth::decode(&[0; 64]).unwrap().encode(), [0; 64]);
    for length in [0, 63, 65] {
        assert_eq!(
            Auth::decode(&vec![0; length]).err(),
            Some(HandshakePayloadError::Length {
                expected: 64,
                actual: length
            })
        );
    }
}
