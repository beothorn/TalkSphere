//! Reclaiming the resources of placements that no longer owe anything.
//!
//! Callers: the provider's maintenance loop and the node's startup. A pass expires placements whose time has come, releases payload
//! bytes of expired and removed placements, forgets expired records entirely and keeps what is still owed: pending and recovered
//! placements until they expire (the recovery cache and status answers need them), and removal tombstones, which answer an exact
//! removal retry for the lifetime of the ledger. A placement locked by a prepared operation is skipped until the coordinator
//! decides. Dependencies: the participant's store, clock and placement locks.
use super::{error::Error, participant::Participant, state::State};

/// What one reclaim pass did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Report {
    /// Placements moved to expired by this pass.
    pub expired: usize,
    /// Payloads released for expired or removed placements.
    pub payloads_released: usize,
    /// Expired placement records forgotten, freeing their slots.
    pub records_dropped: usize,
    /// Placements left alone because a prepared operation holds them.
    pub skipped_locked: usize,
}

impl Participant {
    /// Reclaim expired and removed placements at the current time without touching anything still owed.
    /// # Errors
    /// `Unavailable` before recovery completes or when the clock fails; `Storage` when recording an expiry or forgetting a record
    /// fails. The pass stops there, and running it again is safe because every step is idempotent.
    pub fn reclaim(&self) -> Result<Report, Error> {
        tracing::trace!("Reclaiming finished placements");
        if !self.is_ready() {
            return Err(Error::Unavailable);
        }
        let now = self.now()?;
        // Holding the lock table keeps new operations from preparing on a placement while it is being reclaimed.
        let locks = self.locks();
        let mut report = Report::default();
        for record in self.store().records() {
            if locks.contains_key(&record.id) {
                report.skipped_locked += 1;
                continue;
            }
            let was_active = record.state.is_active();
            let current = super::lifecycle::touch(self.store(), record.id, now)?;
            if was_active && current.state == State::Expired {
                report.expired += 1;
            }
            match current.state {
                State::Expired => {
                    self.store().release_payload(record.id)?;
                    report.payloads_released += 1;
                    self.store().discard(record.id)?;
                    report.records_dropped += 1;
                }
                State::Removed => {
                    // The tombstone stays; only a payload that was somehow left behind is released.
                    self.store().release_payload(record.id)?;
                }
                State::Receiving | State::Pending | State::Recovered => {}
            }
        }
        Ok(report)
    }
}

#[cfg(test)]
#[path = "reclaim_tests.rs"]
mod tests;
