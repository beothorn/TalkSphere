//! Bounded reservation of in-flight store payloads.
//!
//! Callers: the provider when it assesses a store, before any payload body is received. A reservation holds a
//! share of a finite byte and count budget until it is dropped, so concurrent uploads can never exceed the
//! provider's memory bound and a refused or abandoned store leaves nothing reserved behind. An incomplete upload
//! creates no retention obligation and is never persisted.
use super::error::Error;
use std::sync::{Arc, Mutex};

/// The shared budget.
#[derive(Clone)]
pub struct Staging {
    inner: Arc<Mutex<Used>>,
    max_bytes: u64,
    max_count: usize,
}

#[derive(Default)]
struct Used {
    bytes: u64,
    count: usize,
}

/// A held share of the budget; dropping it releases the share.
pub struct Reservation {
    staging: Staging,
    bytes: u64,
}

impl Staging {
    /// A budget of `max_bytes` payload bytes across at most `max_count` simultaneous stores.
    pub fn new(max_bytes: u64, max_count: usize) -> Self {
        tracing::trace!("Creating the store staging budget");
        Self {
            inner: Arc::new(Mutex::new(Used::default())),
            max_bytes,
            max_count,
        }
    }

    /// Reserve room for one store of `bytes` payload bytes.
    /// # Errors
    /// `Limit` when the byte or count budget would be exceeded; nothing is reserved.
    pub fn reserve(&self, bytes: u64) -> Result<Reservation, Error> {
        tracing::trace!("Reserving store staging capacity");
        let mut used = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if used.count >= self.max_count
            || used
                .bytes
                .checked_add(bytes)
                .is_none_or(|b| b > self.max_bytes)
        {
            return Err(Error::Limit);
        }
        used.bytes += bytes;
        used.count += 1;
        drop(used);
        Ok(Reservation {
            staging: self.clone(),
            bytes,
        })
    }

    /// Bytes and stores currently reserved.
    pub fn used(&self) -> (u64, usize) {
        let used = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        (used.bytes, used.count)
    }
}

impl Drop for Reservation {
    // Give the share back, whatever happened to the store it was for.
    fn drop(&mut self) {
        let mut used = self.staging.inner.lock().unwrap_or_else(|e| e.into_inner());
        used.bytes = used.bytes.saturating_sub(self.bytes);
        used.count = used.count.saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Reservations count against both budgets, are refused at the boundary without effect and are released on drop.
    #[test]
    fn reservations_are_bounded_and_released() {
        let staging = Staging::new(100, 2);
        let a = staging.reserve(60).unwrap();
        assert_eq!(staging.reserve(41).err(), Some(Error::Limit));
        let b = staging.reserve(40).unwrap();
        assert_eq!(staging.used(), (100, 2));
        assert_eq!(
            staging.reserve(0).err(),
            Some(Error::Limit),
            "the count budget is separate"
        );
        drop(a);
        assert_eq!(staging.used(), (40, 1));
        let c = staging.reserve(60).unwrap();
        drop((b, c));
        assert_eq!(staging.used(), (0, 0));
        assert_eq!(staging.reserve(u64::MAX).err(), Some(Error::Limit));
    }

    // Many threads reserving and dropping never exceed the budget and always return to zero.
    #[test]
    fn concurrent_use_respects_the_budget() {
        let staging = Staging::new(10, 100);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let staging = staging.clone();
                scope.spawn(move || {
                    for _ in 0..200 {
                        if let Ok(r) = staging.reserve(3) {
                            assert!(staging.used().0 <= 10);
                            drop(r);
                        }
                    }
                });
            }
        });
        assert_eq!(staging.used(), (0, 0));
    }
}
