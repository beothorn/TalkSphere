//! Exact bounded SHA-256 over bytes or caller-ordered chunks, for all modules.

use crate::modules::cryptography::error::CryptoError;
use sha2::{Digest, Sha256};

/// A bounded ordered hash operation; finalization consumes the operation.
/// ```compile_fail
/// use talksphere::modules::cryptography::primitives::hash::Sha256State;
/// let state = Sha256State::new(10);
/// state.finish();
/// state.finish();
/// ```
pub struct Sha256State {
    state: Sha256,
    remaining: usize,
}
impl Sha256State {
    /// Begin an operation with a local total-byte budget, including zero for empty input.
    pub fn new(maximum_bytes: usize) -> Self {
        tracing::trace!("Beginning an exact ordered SHA-256 operation");
        Self {
            state: Sha256::new(),
            remaining: maximum_bytes,
        }
    }
    /// Append exact bytes in order, accepting empty chunks without normalization.
    /// # Errors
    /// Returns `ResourceLimit` before changing state if the remaining budget is exceeded.
    pub fn update(&mut self, bytes: &[u8]) -> Result<(), CryptoError> {
        tracing::trace!("Hashing a bounded input chunk");
        if bytes.len() > self.remaining {
            return Err(CryptoError::ResourceLimit);
        }
        self.state.update(bytes);
        self.remaining -= bytes.len();
        Ok(())
    }
    /// Consume the operation and return the exact 32-byte digest.
    pub fn finish(self) -> [u8; 32] {
        tracing::trace!("Finalizing an ordered SHA-256 operation");
        self.state.finalize().into()
    }
}

/// Hash exact bytes within a caller-specified local total-byte budget.
/// # Errors
/// Returns `ResourceLimit` before hashing an oversized input.
pub fn sha256(bytes: &[u8], maximum_bytes: usize) -> Result<[u8; 32], CryptoError> {
    tracing::trace!("Hashing exact bounded bytes");
    let mut state = Sha256State::new(maximum_bytes);
    state.update(bytes)?;
    Ok(state.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::cryptography::test_support::hex;
    // CR-01 compares literal known answers across every partition of abc, including empties.
    #[test]
    fn hashes_match_known_answers_and_all_chunk_partitions() {
        assert_eq!(
            sha256(b"", 0).unwrap().as_slice(),
            hex("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        );
        for first in 0..=3 {
            for second in first..=3 {
                let mut state = Sha256State::new(3);
                state.update(&b"abc"[..first]).unwrap();
                state.update(&b"abc"[first..second]).unwrap();
                state.update(&b"abc"[second..]).unwrap();
                assert_eq!(
                    state.finish().as_slice(),
                    hex("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
                );
            }
        }
    }
    // A rejected chunk changes no accepted hash state or budget; a valid retry still works.
    #[test]
    fn failed_updates_are_atomic() {
        assert_eq!(sha256(b"a", 0), Err(CryptoError::ResourceLimit));
        let mut state = Sha256State::new(3);
        state.update(b"a").unwrap();
        assert_eq!(state.update(b"bcd"), Err(CryptoError::ResourceLimit));
        state.update(b"bc").unwrap();
        assert_eq!(state.finish(), sha256(b"abc", 3).unwrap());
    }
}
