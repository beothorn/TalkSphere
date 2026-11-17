//! Bounded terminal-resource reclamation keeps original results and ID history while waiting for accepted byte lifetimes.
use super::{Gate, usage};
use crate::modules::{
    services::Error,
    storage::{
        contract::{LeaseId, State, Time},
        effect::Effect,
        image::Stage,
    },
};
use std::collections::BTreeSet;

/// One bounded cleanup observation; counts describe confirmed local effects, not refunds or secure erasure.
#[derive(Debug, PartialEq, Eq)]
pub struct Cleanup {
    /// Active leases durably marked expired during this call.
    pub expired: usize,
    /// Terminal byte objects considered in this batch, including deferred pins.
    pub examined: usize,
    /// Objects confirmed durably absent, including idempotent confirmation after an interrupted prior cleanup.
    pub reclaimed: usize,
    /// Objects deferred because an accepted immutable descriptor remains alive.
    pub pinned: usize,
    /// Last examined lease ID when more eligible IDs remain; pass it as after to continue, or None to start a fresh sweep.
    pub next: Option<LeaseId>,
}
impl Gate {
    /// Advance durable retention and reclaim at most maximum_attempts eligible terminal byte objects after an optional cursor.
    /// Call only with trusted local time; unresolved mutation targets and live read pins are protected.
    /// Results, unique lease IDs and original coordinator history are never evicted by cleanup.
    /// # Errors
    /// Invalid rejects a zero batch, Unavailable closed admission, and clock/storage errors preserve their meaning.
    /// Unknown closes admission and retains holds until original owner recovery after possible persistence/deletion.
    pub fn collect(
        &self,
        now: Time,
        reboot_elapsed: Option<u64>,
        maximum_attempts: usize,
        after: Option<LeaseId>,
    ) -> Result<Cleanup, Error> {
        tracing::trace!(
            "Collecting bounded expired or removed Storage bytes without losing original history"
        );
        if maximum_attempts == 0 {
            return Err(Error::Invalid);
        }
        let mut admission = self.admission.lock().map_err(|_| Error::Unavailable)?;
        if !admission.ready {
            return Err(Error::Unavailable);
        }
        let result = (|| {
            let (image, expired) = self.journal.cleanup_time(now, reboot_elapsed)?;
            let limits = self.journal.image_limits();
            let mut protected = BTreeSet::new();
            for record in &image.operations {
                if matches!(record.status, Stage::Staged | Stage::Prepared(_))
                    && let Some(id) =
                        Effect::decode(record.completion.effect.bytes(), image.provider, limits)?
                            .resource()
                {
                    protected.insert(id);
                }
            }
            let mut ids = image
                .leases
                .iter()
                .filter(|lease| {
                    matches!(lease.state, State::Expired | State::Removed)
                        && !protected.contains(&lease.id)
                        && after.is_none_or(|cursor| lease.id > cursor)
                })
                .map(|lease| lease.id)
                .collect::<Vec<_>>();
            ids.sort_unstable();
            let count = ids.len().min(maximum_attempts);
            let mut cleanup = Cleanup {
                expired,
                examined: count,
                reclaimed: 0,
                pinned: 0,
                next: if ids.len() > count {
                    ids.get(count - 1).copied()
                } else {
                    None
                },
            };
            for id in ids.into_iter().take(count) {
                if self.bytes.reclaim(id)? {
                    cleanup.reclaimed += 1;
                } else {
                    cleanup.pinned += 1;
                }
            }
            let retained = usage::objects(&self.bytes, &image, limits)?;
            let quota = admission.quota.as_ref().ok_or(Error::Unknown)?;
            for record in &image.operations {
                if matches!(record.status, Stage::Committed(_))
                    && matches!(
                        Effect::decode(record.completion.effect.bytes(), image.provider, limits)?,
                        Effect::Create(_)
                    )
                {
                    quota
                        .transition(
                            record.key,
                            usage::record_usage(&image, record, limits, &retained)?,
                        )
                        .map_err(|_| Error::Unknown)?;
                }
            }
            Ok(cleanup)
        })();
        if matches!(result, Err(Error::Unknown)) {
            admission.ready = false;
        }
        result
    }
}
#[cfg(test)]
#[path = "../tests/cleanup.rs"]
mod tests;
