//! X25519 agreement rejects non-contributory results before exposing protected secrets.

use super::super::{
    error::CryptoError,
    secrets::{Ephemeral, Persistent, Secret, Shared},
};
use zeroize::Zeroizing;

/// Consume one ephemeral capability and return only a protected shared result.
/// # Errors
/// Rejects malformed remote length or an all-zero result, releasing consumed material.
pub fn ephemeral(key: Secret<Ephemeral>, remote: &[u8]) -> Result<Secret<Shared>, CryptoError> {
    tracing::trace!("Consuming ephemeral authority for X25519 agreement");
    agree(&key.bytes, remote)
}

/// Borrow persistent authority for X25519 agreement without consuming it.
/// # Errors
/// Rejects malformed remote length or an all-zero result; no raw shared bytes escape.
pub fn persistent(key: &Secret<Persistent>, remote: &[u8]) -> Result<Secret<Shared>, CryptoError> {
    tracing::trace!("Using persistent authority for X25519 agreement");
    agree(&key.bytes, remote)
}

// Delegate scalar clamping and multiplication to the backend and reject zero in constant time.
fn agree(bytes: &[u8; 32], remote: &[u8]) -> Result<Secret<Shared>, CryptoError> {
    tracing::trace!("Checking an X25519 agreement result");
    let remote: [u8; 32] = remote.try_into().map_err(|_| CryptoError::InvalidInput)?;
    let key = x25519_dalek::StaticSecret::from(*bytes);
    let result = key.diffie_hellman(&x25519_dalek::PublicKey::from(remote));
    if !result.was_contributory() {
        return Err(CryptoError::AuthenticationFailed);
    }
    Ok(Secret::new(Zeroizing::new(result.to_bytes())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::cryptography::test_support::{ReleaseProbe, envelope};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    // Both roles reproduce the pinned shared result; persistent authority survives reuse.
    #[test]
    fn pinned_agreement_matches_both_directions() {
        let ephemeral_key = Secret::new(Zeroizing::new(
            envelope("raw ephemeral X25519 private-key input")
                .try_into()
                .unwrap(),
        ));
        let persistent_key = Secret::new(Zeroizing::new(
            envelope("recipient raw persistent X25519 private-key input")
                .try_into()
                .unwrap(),
        ));
        let shared = ephemeral(
            ephemeral_key,
            &envelope("recipient persistent X25519 public key"),
        )
        .unwrap();
        assert_eq!(shared.bytes.as_slice(), envelope("X25519 shared secret"));
        for _ in 0..2 {
            let shared =
                persistent(&persistent_key, &envelope("ephemeral X25519 public key")).unwrap();
            assert_eq!(shared.bytes.as_slice(), envelope("X25519 shared secret"));
        }
    }
    // Low-order public inputs produce zero even when multiplication itself succeeds.
    #[test]
    fn zero_results_and_malformed_inputs_release_ephemeral_keys() {
        let count = Arc::new(AtomicUsize::new(0));
        let mut low_order = [0; 32];
        low_order[0] = 1;
        for remote in [vec![0; 32], low_order.to_vec(), vec![2; 31], vec![2; 33]] {
            let mut key = Secret::new(Zeroizing::new([9; 32]));
            key.release = Some(ReleaseProbe(count.clone()));
            assert!(ephemeral(key, &remote).is_err());
        }
        assert_eq!(count.load(Ordering::SeqCst), 4);
    }
}
