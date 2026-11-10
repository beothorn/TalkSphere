//! Trusted monotonic check timing shares a stable local epoch; peer timestamps never control deadlines.
use super::Error;
use crate::modules::services::checks::Stamp;
use std::time::Instant;
/// Platform timing dependency, independently scriptable without sleeps.
pub trait Clock: Send + Sync {
    /// Observe one local instant and comparable epoch/millisecond stamp, or fail without inventing timing evidence.
    fn mark(&self) -> Result<(Instant, Stamp), Error>;
}
/// Production monotonic clock; restarting creates a fresh epoch rather than comparing unrelated counters.
pub struct SystemClock {
    base: Instant,
    epoch: [u8; 32],
}
impl SystemClock {
    /// Create an OS-random clock epoch and monotonic origin.
    /// # Errors
    /// Randomness failure is Unavailable; clocks never fall back to a reused predictable epoch.
    pub fn new() -> Result<Self, Error> {
        tracing::trace!("Creating a trusted local check clock epoch");
        let mut epoch = [0; 32];
        getrandom::fill(&mut epoch).map_err(|_| Error::Unavailable)?;
        Ok(Self {
            base: Instant::now(),
            epoch,
        })
    }
}
impl Clock for SystemClock {
    // Capture one instant and derive its exact elapsed milliseconds without a wall clock or peer-supplied time.
    fn mark(&self) -> Result<(Instant, Stamp), Error> {
        tracing::trace!("Observing trusted local evidence timing");
        let now = Instant::now();
        let elapsed = now
            .checked_duration_since(self.base)
            .ok_or(Error::Unknown)?;
        Ok((
            now,
            Stamp {
                epoch: self.epoch,
                milliseconds: u64::try_from(elapsed.as_millis()).map_err(|_| Error::Limit)?,
            },
        ))
    }
}
