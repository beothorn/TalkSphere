//! Endpoint admission reserves aggregate logical value/body capacity before any stream callback can create effects.
use super::*;
use std::{collections::BTreeMap, sync::Mutex};

/// Aggregate transient endpoint budgets, separate from Transport framing and durable owner storage.
#[derive(Clone, Copy, Debug)]
pub struct Budgets {
    /// Logical value bytes reserved across active handlers; this is not a process RSS limit.
    pub value_bytes: usize,
    /// Detached body references reserved across active handlers.
    pub bodies: usize,
    /// Maximum active handlers for one authenticated remote identity, across all its connections.
    pub per_peer: usize,
}
impl Default for Budgets {
    // Finite operator defaults reserve space for four full-size default exchanges.
    fn default() -> Self {
        tracing::trace!("Selecting aggregate endpoint admission budgets");
        Self {
            value_bytes: 268_435_456,
            bodies: 65_536,
            per_peer: 4,
        }
    }
}
struct Usage {
    count: usize,
    peers: BTreeMap<IdentityId, usize>,
}
pub(super) struct Pool {
    maximum: usize,
    per_peer: usize,
    usage: Mutex<Usage>,
}
struct Permit {
    pool: Arc<Pool>,
    peer: IdentityId,
}
impl Pool {
    // Reserve two logical directions plus transient encoding/assembly copies before callbacks; owners have independent journal budgets.
    pub(super) fn new(limits: Limits, budgets: Budgets) -> Result<Arc<Self>, Error> {
        tracing::trace!("Validating aggregate endpoint value and body capacity");
        if budgets.value_bytes == 0 || budgets.bodies == 0 || budgets.per_peer == 0 {
            return Err(Error::Invalid);
        }
        let bytes = limits.state_bytes.checked_mul(4).ok_or(Error::Limit)?;
        // Detached values must exceed the protocol's 4096-byte inline threshold.
        let bodies = limits
            .bodies
            .min(limits.state_bytes / 4097)
            .checked_mul(2)
            .ok_or(Error::Limit)?
            .max(1);
        let maximum = limits
            .exchanges
            .min(budgets.value_bytes / bytes)
            .min(budgets.bodies / bodies);
        if maximum == 0 {
            return Err(Error::Limit);
        }
        Ok(Arc::new(Self {
            maximum,
            per_peer: budgets.per_peer,
            usage: Mutex::new(Usage {
                count: 0,
                peers: BTreeMap::new(),
            }),
        }))
    }
    // Acquire atomically across every endpoint clone, before constructing a handler or reserving financial state.
    fn acquire(self: &Arc<Self>, peer: IdentityId) -> Result<Permit, Error> {
        tracing::trace!("Reserving aggregate transient endpoint capacity");
        let mut usage = self.usage.lock().map_err(|_| Error::Unavailable)?;
        if usage.count >= self.maximum
            || usage.peers.get(&peer).copied().unwrap_or(0) >= self.per_peer
        {
            return Err(Error::Limit);
        }
        usage.count += 1;
        *usage.peers.entry(peer).or_default() += 1;
        Ok(Permit {
            pool: self.clone(),
            peer,
        })
    }
    // Keep the reservation attached to the callback owner until its entire worker-owned handler is dropped.
    pub(super) fn open(
        self: &Arc<Self>,
        peer: IdentityId,
        create: impl FnOnce() -> Result<Box<dyn Handler>, Error>,
    ) -> Result<Box<dyn Handler>, Error> {
        tracing::trace!("Admitting one bounded endpoint handler");
        let permit = self.acquire(peer)?;
        Ok(Box::new(Guarded {
            inner: create()?,
            _permit: permit,
        }))
    }
}
impl Drop for Permit {
    // Releasing transient admission never aborts, retries or evicts durable accepted operations.
    fn drop(&mut self) {
        tracing::trace!("Releasing transient endpoint admission after handler cleanup");
        if let Ok(mut usage) = self.pool.usage.lock() {
            usage.count -= 1;
            if let Some(count) = usage.peers.get_mut(&self.peer) {
                *count -= 1;
                if *count == 0 {
                    usage.peers.remove(&self.peer);
                }
            }
        }
    }
}
struct Guarded {
    inner: Box<dyn Handler>,
    _permit: Permit,
}
impl Handler for Guarded {
    // Retain admission throughout the underlying record callback, including dependency waits and late completion.
    fn record(&mut self, record: Record, output: &mut Output) -> Result<(), Error> {
        tracing::trace!("Dispatching a record under retained endpoint admission");
        self.inner.record(record, output)
    }
    // Preserve the underlying normal END gate under the original admission reservation.
    fn remote_end(&mut self, output: &mut Output) -> Result<(), Error> {
        tracing::trace!("Dispatching remote END under retained endpoint admission");
        self.inner.remote_end(output)
    }
    // Backpressure does not release or reacquire transient capacity.
    fn writable(&mut self, output: &mut Output) -> Result<(), Error> {
        tracing::trace!("Resuming output under retained endpoint admission");
        self.inner.writable(output)
    }
    // Legacy cleanup preserves durable owner's own conservative financial resolution.
    fn closed(
        &mut self,
        outcome: crate::modules::transport::api::StreamOutcome,
    ) -> Result<(), Error> {
        tracing::trace!("Closing an admitted endpoint handler");
        self.inner.closed(outcome)
    }
    // Preserve original closure time and write-submission evidence without replacing the richer callback.
    fn closed_observed(&mut self, closure: peer::engine::bridge::Closure) -> Result<(), Error> {
        tracing::trace!("Closing an admitted endpoint with original Transport observations");
        self.inner.closed_observed(closure)
    }
}
