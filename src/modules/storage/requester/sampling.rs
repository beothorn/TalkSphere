//! Optional requester sampling chooses uniformly distributed uint64 offsets with bounded secure rejection draws.
use crate::modules::{
    cryptography::{ports::Entropy, random::random_bytes},
    services::Error,
};

/// Choose one unbiased offset in 0..size_bytes using fresh secure entropy, never a preannounced provider-selected value.
/// `maximum_draws` is a positive local effort bound; rejection avoids modulo bias even near uint64::MAX.
/// # Errors
/// Invalid rejects zero size/effort, Unavailable entropy failure, and Limit exhaustion of the configured draw bound.
pub fn offset(
    entropy: &mut impl Entropy,
    size_bytes: u64,
    maximum_draws: usize,
) -> Result<u64, Error> {
    tracing::trace!("Sampling a private Storage check offset using bounded secure rejection draws");
    if size_bytes == 0 || maximum_draws == 0 {
        return Err(Error::Invalid);
    }
    // 2^64 mod size is represented without a wider integer by unsigned wrapping negation before modulo.
    let threshold = size_bytes.wrapping_neg() % size_bytes;
    for _ in 0..maximum_draws {
        let random = random_bytes::<8>(entropy).map_err(|_| Error::Unavailable)?;
        let number = u64::from_be_bytes(*random);
        if number >= threshold {
            return Ok(number % size_bytes);
        }
    }
    Err(Error::Limit)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::cryptography::test_support::ScriptedEntropy;
    use std::collections::VecDeque;
    // The lowest rejected bucket is discarded rather than biased into offset zero; uint64 extrema require no object allocation.
    #[test]
    fn rejection_and_uint64_boundaries() {
        let mut entropy = ScriptedEntropy(VecDeque::from([
            Ok(0u64.to_be_bytes().to_vec()),
            Ok(5u64.to_be_bytes().to_vec()),
        ]));
        assert_eq!(offset(&mut entropy, 3, 2), Ok(2));
        let mut entropy = ScriptedEntropy(VecDeque::from([Ok(u64::MAX.to_be_bytes().to_vec())]));
        assert_eq!(offset(&mut entropy, u64::MAX, 1), Ok(0));
        let mut entropy = ScriptedEntropy(VecDeque::from([Ok(u64::MAX.to_be_bytes().to_vec())]));
        assert_eq!(offset(&mut entropy, 1, 1), Ok(0));
    }
    // A failed/partial entropy source or exhausted rejection budget cannot manufacture a predictable fallback offset.
    #[test]
    fn invalid_effort_entropy_and_rejection_exhaustion() {
        let mut empty = ScriptedEntropy(VecDeque::new());
        assert_eq!(offset(&mut empty, 0, 1), Err(Error::Invalid));
        assert_eq!(offset(&mut empty, 1, 0), Err(Error::Invalid));
        assert_eq!(offset(&mut empty, 1, 1), Err(Error::Unavailable));
        let mut entropy = ScriptedEntropy(VecDeque::from([Ok(0u64.to_be_bytes().to_vec())]));
        assert_eq!(offset(&mut entropy, 3, 1), Err(Error::Limit));
    }
}
