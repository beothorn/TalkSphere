//! Purpose-scoped owned secret material for trusted local key-store adapters.

use std::marker::PhantomData;
use zeroize::{Zeroize, Zeroizing};

/// Ed25519 signing seed purpose.
pub enum Signing {}
/// Persistent X25519 private input purpose.
pub enum Persistent {}
/// Single-use ephemeral X25519 private input purpose.
pub enum Ephemeral {}
/// Raw agreement result requiring purpose-specific derivation.
pub enum Shared {}
/// HKDF extracted pseudorandom key purpose.
pub enum Prk {}
/// Content encryption key purpose.
pub enum Content {}
/// Transport integrity key purpose, distinct from content keys.
pub enum Integrity {}

/// An owned 32-byte secret with no public byte access, Clone, Copy or Debug.
///
/// Import is restricted by application composition to authorized local callers;
/// it is not a peer-facing API or a replacement for protected persistent storage.
/// Algorithm-specific validation occurs when the capability is used.
/// ```compile_fail
/// use talksphere::modules::cryptography::secrets::{Secret, Signing};
/// fn expose(key: Secret<Signing>) { println!("{key:?}"); }
/// ```
pub struct Secret<P> {
    pub(super) bytes: Zeroizing<[u8; 32]>,
    purpose: PhantomData<P>,
    #[cfg(test)]
    pub(crate) release: Option<super::test_support::ReleaseProbe>,
}

impl<P> Secret<P> {
    /// Take ownership of protected fixed-size material from an authorized caller.
    ///
    /// `bytes` is a seed, private input or derived value appropriate to `P`.
    /// Moving a value cannot guarantee erasure of compiler/register copies;
    /// callers must protect their own prior copies. No I/O or validation occurs.
    pub fn new(bytes: Zeroizing<[u8; 32]>) -> Self {
        tracing::trace!("Taking ownership of purpose-scoped secret material");
        Self {
            bytes,
            purpose: PhantomData,
            #[cfg(test)]
            release: None,
        }
    }
}

impl<P> Drop for Secret<P> {
    // Erase owned bytes before releasing the capability; never log its contents.
    fn drop(&mut self) {
        tracing::trace!("Releasing purpose-scoped secret material");
        self.bytes.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::cryptography::{error::CryptoError, test_support::ReleaseProbe};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    // Capability release is observable on normal drop and an early failure return.
    #[test]
    fn release_on_success_and_failure() {
        let count = Arc::new(AtomicUsize::new(0));
        let mut key = Secret::<Signing>::new(Zeroizing::new([9; 32]));
        key.release = Some(ReleaseProbe(count.clone()));
        drop(key);
        assert_eq!(count.load(Ordering::SeqCst), 1);
        let mut key = Secret::<Ephemeral>::new(Zeroizing::new([7; 32]));
        key.release = Some(ReleaseProbe(count.clone()));
        let result: Result<(), CryptoError> = (|| {
            let _owned = key;
            Err(CryptoError::Unavailable)?
        })();
        assert_eq!(result, Err(CryptoError::Unavailable));
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }
}
