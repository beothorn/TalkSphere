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
    /// Derive a comparable stamp from an original trusted local observation, preserving asynchronous queue delay.
    /// # Errors
    /// Before-epoch or future observations are Invalid; unrepresentable elapsed milliseconds are Limit.
    pub fn stamp_at(&self, observed: Instant) -> Result<Stamp, Error> {
        tracing::trace!("Binding an original local observation to the stable check clock epoch");
        if observed > Instant::now() {
            return Err(Error::Invalid);
        }
        let elapsed = observed
            .checked_duration_since(self.base)
            .ok_or(Error::Invalid)?;
        Ok(Stamp {
            epoch: self.epoch,
            milliseconds: u64::try_from(elapsed.as_millis()).map_err(|_| Error::Limit)?,
        })
    }
}
impl Clock for SystemClock {
    // Capture one instant and derive its exact elapsed milliseconds without a wall clock or peer-supplied time.
    fn mark(&self) -> Result<(Instant, Stamp), Error> {
        tracing::trace!("Observing trusted local evidence timing");
        let now = Instant::now();
        Ok((now, self.stamp_at(now)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Deferred workers recover exactly the original stamp; observations outside this clock's real epoch are rejected.
    #[test]
    fn original_observations_preserve_stamp_without_inventing_another_epoch() {
        let clock = SystemClock::new().unwrap();
        let (instant, stamp) = clock.mark().unwrap();
        assert_eq!(clock.stamp_at(instant), Ok(stamp));
        assert_eq!(
            clock.stamp_at(Instant::now() + std::time::Duration::from_secs(1)),
            Err(Error::Invalid)
        );
        assert_eq!(
            clock.stamp_at(clock.base - std::time::Duration::from_nanos(1)),
            Err(Error::Invalid)
        );
    }
}
