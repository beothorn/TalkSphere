//! A small deterministic random number generator for simulations.
//!
//! SplitMix64: one 64-bit state, fully determined by the seed, no external entropy and no
//! dependence on the platform, so the same seed reproduces the same run everywhere. It is for
//! simulation only and must never be used for keys, identifiers or any security decision.

/// Deterministic generator.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    /// A generator fully determined by `seed`.
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A uniform value in `[0, 1)`.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// A uniform value in `0..n`; `n` must be nonzero.
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n.max(1)
    }

    /// True with probability `p` (clamped to `[0, 1]`).
    pub fn chance(&mut self, p: f64) -> bool {
        self.unit() < p.clamp(0.0, 1.0)
    }

    /// An exponentially distributed duration with the given mean, in seconds, at least one second.
    pub fn exponential(&mut self, mean: f64) -> u64 {
        let u = 1.0 - self.unit();
        ((-u.ln() * mean).round() as u64).max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The same seed gives the same sequence and different seeds differ.
    #[test]
    fn the_sequence_is_determined_by_the_seed() {
        let (mut a, mut b, mut c) = (Rng::new(7), Rng::new(7), Rng::new(8));
        let first: Vec<u64> = (0..5).map(|_| a.next_u64()).collect();
        assert_eq!(first, (0..5).map(|_| b.next_u64()).collect::<Vec<_>>());
        assert_ne!(first, (0..5).map(|_| c.next_u64()).collect::<Vec<_>>());
        // The published SplitMix64 test vector for seed 0.
        assert_eq!(Rng::new(0).next_u64(), 0xE220_A839_7B1D_CDAF);
    }

    // Values stay in range and the exponential mean is close to the requested one.
    #[test]
    fn ranges_and_means_hold() {
        let mut rng = Rng::new(1);
        for _ in 0..1000 {
            assert!((0.0..1.0).contains(&rng.unit()));
            assert!(rng.below(10) < 10);
        }
        assert_eq!(rng.below(0), 0);
        let mean = (0..20_000)
            .map(|_| rng.exponential(100.0) as f64)
            .sum::<f64>()
            / 20_000.0;
        assert!((95.0..105.0).contains(&mean), "{mean}");
        assert!(!rng.chance(0.0) && rng.chance(1.0) && rng.chance(2.0));
        let hits = (0..10_000).filter(|_| rng.chance(0.25)).count();
        assert!((2300..2700).contains(&hits), "{hits}");
    }
}
