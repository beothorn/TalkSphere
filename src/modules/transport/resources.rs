//! Shared finite admission budgets; reservations follow ownership until bytes or sockets are released.
use super::config::ResourceLimits;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

/// Cloneable instance-wide policy and counters shared by every admitted connection.
#[derive(Clone)]
pub struct Resources {
    limits: ResourceLimits,
    connections: Arc<Counter>,
    outbound: Arc<Counter>,
}
struct Counter {
    used: AtomicUsize,
    limit: usize,
}

/// A reservation releases capacity only after its final ownership clone is dropped.
#[derive(Clone)]
pub(crate) struct Reservation(Arc<Lease>);
struct Lease {
    counter: Arc<Counter>,
    amount: usize,
}

impl Resources {
    /// Create counters from previously validated Transport configuration limits.
    pub fn new(limits: ResourceLimits) -> Self {
        tracing::trace!(
            function = "Resources::new",
            "Creating shared Transport resource budgets"
        );
        Self {
            limits,
            connections: Arc::new(Counter {
                used: AtomicUsize::new(0),
                limit: limits.connections,
            }),
            outbound: Arc::new(Counter {
                used: AtomicUsize::new(0),
                limit: limits.outbound_bytes,
            }),
        }
    }
    /// Return the policy enforced by all clones of this resource instance.
    pub fn limits(&self) -> ResourceLimits {
        tracing::trace!(
            function = "Resources::limits",
            "Reading shared resource policy"
        );
        self.limits
    }
    /// Return live admitted sockets/attempts for diagnostics and admission tests.
    pub fn connections(&self) -> usize {
        tracing::trace!(
            function = "Resources::connections",
            "Reading admitted connection count"
        );
        self.connections.used.load(Ordering::Relaxed)
    }
    /// Return queued/in-flight serialized bytes, retained until successful output or discard.
    pub fn outbound_bytes(&self) -> usize {
        tracing::trace!(
            function = "Resources::outbound_bytes",
            "Reading retained output bytes"
        );
        self.outbound.used.load(Ordering::Relaxed)
    }
    // Admit a socket before credentials or framing buffers are allocated.
    pub(crate) fn connection(&self) -> Option<Reservation> {
        tracing::trace!(
            function = "Resources::connection",
            "Reserving connection capacity"
        );
        reserve(&self.connections, 1)
    }
    // Atomically reserve serialized bytes across all connections in this instance.
    pub(crate) fn outbound(&self, amount: usize) -> Option<Reservation> {
        tracing::trace!(
            function = "Resources::outbound",
            "Reserving serialized output capacity"
        );
        reserve(&self.outbound, amount)
    }
}

// Checked atomic admission prevents overflow and oversubscription by concurrent owners.
fn reserve(counter: &Arc<Counter>, amount: usize) -> Option<Reservation> {
    tracing::trace!(
        function = "resources::reserve",
        "Checking finite admission capacity"
    );
    counter
        .used
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
            used.checked_add(amount)
                .filter(|next| *next <= counter.limit)
        })
        .ok()?;
    Some(Reservation(Arc::new(Lease {
        counter: counter.clone(),
        amount,
    })))
}

impl Drop for Lease {
    // Final ownership release makes capacity available without a separate recovery call.
    fn drop(&mut self) {
        tracing::trace!(function = "Lease::drop", "Releasing resource reservation");
        self.counter.used.fetch_sub(self.amount, Ordering::Relaxed);
    }
}

// Read the ownership field when cloning so the opaque guard has an explicit retained purpose.
impl Reservation {
    pub(crate) fn retain(&self) -> Self {
        tracing::trace!(
            function = "Reservation::retain",
            "Retaining in-flight output capacity"
        );
        Self(self.0.clone())
    }
}

#[cfg(test)]
#[path = "resources/tests.rs"]
mod tests;
