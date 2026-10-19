//! Secure platform entropy and bounded zeroizing buffers for key generation.

use super::{error::CryptoError, ports::Entropy};
use zeroize::Zeroizing;

/// Maximum single entropy request; sufficient for every CR2SE key and nonce.
pub const MAX_RANDOM_BYTES: usize = 4096;

/// Platform CSPRNG adapter selected explicitly by application composition.
pub struct SystemEntropy;
impl Entropy for SystemEntropy {
    /// Fill at most 4096 bytes from platform secure randomness, including empty requests.
    ///
    /// # Errors
    /// Returns `ResourceLimit` for larger buffers or `Unavailable` on platform failure.
    fn fill(&mut self, output: &mut [u8]) -> Result<(), CryptoError> {
        tracing::trace!("Requesting platform secure randomness");
        if output.len() > MAX_RANDOM_BYTES {
            return Err(CryptoError::ResourceLimit);
        }
        getrandom::fill(output).map_err(|_| CryptoError::Unavailable)
    }
}

/// Obtain a fixed-size protected buffer from an authorized entropy dependency.
///
/// The caller owns successful output; partial failure buffers erase on drop.
/// Deterministic adapters must only be supplied in tests, never normal composition.
/// # Errors
/// Returns `ResourceLimit` above 4096 bytes or propagates entropy failure.
pub fn random_bytes<const N: usize>(
    entropy: &mut impl Entropy,
) -> Result<Zeroizing<[u8; N]>, CryptoError> {
    tracing::trace!("Obtaining bounded protected random material");
    if N > MAX_RANDOM_BYTES {
        return Err(CryptoError::ResourceLimit);
    }
    let mut bytes = Zeroizing::new([0; N]);
    entropy.fill(bytes.as_mut())?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::cryptography::test_support::ScriptedEntropy;
    use std::collections::VecDeque;

    struct PartialFailure;
    impl Entropy for PartialFailure {
        // Exercise a dependency that mutates its output before failing.
        fn fill(&mut self, output: &mut [u8]) -> Result<(), CryptoError> {
            output.fill(0xab);
            Err(CryptoError::Unavailable)
        }
    }
    // Fresh calls consume independent requests; partial failures never return bytes.
    #[test]
    fn protected_requests_propagate_failure_and_enforce_bounds() {
        let mut entropy = ScriptedEntropy(VecDeque::from([Ok(vec![1; 32]), Ok(vec![2; 32])]));
        assert_eq!(*random_bytes::<32>(&mut entropy).unwrap(), [1; 32]);
        assert_eq!(*random_bytes::<32>(&mut entropy).unwrap(), [2; 32]);
        assert!(matches!(
            random_bytes::<32>(&mut PartialFailure),
            Err(CryptoError::Unavailable)
        ));
        assert!(matches!(
            random_bytes::<4097>(&mut entropy),
            Err(CryptoError::ResourceLimit)
        ));
        assert!(entropy.0.is_empty());
    }
    // Platform access succeeds without asserting probabilistic properties of output.
    #[test]
    fn platform_smoke_and_limits() {
        let mut entropy = SystemEntropy;
        assert!(random_bytes::<32>(&mut entropy).is_ok());
        assert!(entropy.fill(&mut []).is_ok());
        assert_eq!(
            entropy.fill(&mut [0; 4097]),
            Err(CryptoError::ResourceLimit)
        );
    }
}
