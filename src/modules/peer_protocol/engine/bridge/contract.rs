//! Trusted handler contracts and finite adapter budgets; no peer data installs callbacks.
use super::{
    Association, CONTROL_MAX, Direction, Duration, Error, IdentityId, OperationKey, Outbound,
    Record, StreamOutcome,
};
/// Authenticated endpoints and unique stream instance, before application requester roles are known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scope {
    /// Identity of this receiving node.
    pub local: IdentityId,
    /// Verified opposite endpoint, never read from application JSON.
    pub peer: IdentityId,
    /// Random process domain and nonwrapping authenticated connection counter.
    pub connection: [u8; 32],
    /// Transport-issued nonzero stream number within that connection.
    pub stream: u32,
    /// Original monotonic admission deadline, preserved across worker queue delays.
    pub deadline: std::time::Instant,
}
impl Scope {
    /// Bind a requester after validating provided/wanted roles against selected terms.
    /// `requester` must be one of the authenticated endpoints; the opener alone does not select it.
    /// # Errors
    /// Denied rejects an unrelated identity; the handler must still prove application roles.
    pub fn association(self, requester: IdentityId) -> Result<Association, Error> {
        tracing::trace!("Binding selected requester roles to an authenticated peer stream");
        if requester != self.local && requester != self.peer {
            return Err(Error::Denied);
        }
        Ok(Association {
            local: self.local,
            peer: self.peer,
            operation: OperationKey {
                requester,
                connection: self.connection,
                stream: self.stream,
            },
        })
    }
}
/// Trusted per-stream application owner, called only on dependency worker threads.
/// Implementations must validate wire ordering, semantic authority and durable effects
/// through the public module contracts; framing alone is not an authorized invocation.
pub trait Handler: Send {
    /// Consume a complete framed record under the association given to the factory.
    /// # Errors
    /// A failure terminates this stream; it never implies an accepted effect was rolled back.
    fn record(&mut self, record: Record, output: &mut Output) -> Result<(), Error>;
    /// Observe a complete remote direction, after the decoder rejects partial records.
    /// # Errors
    /// Missing application terminal markers must fail rather than expose provisional results.
    fn remote_end(&mut self, output: &mut Output) -> Result<(), Error>;
    /// Refill a drained output queue; called after local queue admission, not remote receipt.
    /// # Errors
    /// Dependency failure terminates this stream without automatic retry.
    fn writable(&mut self, output: &mut Output) -> Result<(), Error>;
    /// Observe actual Transport closure, including late closure after a dependency call.
    /// # Errors
    /// Preserve unknown accepted effects and owner recovery obligations on failure.
    fn closed(&mut self, outcome: StreamOutcome) -> Result<(), Error>;
}
/// Trusted installation boundary; remote bytes cannot install or select arbitrary callbacks.
pub trait Factory: Send + Sync + 'static {
    /// Create one owner for authenticated endpoints and their original connection/stream scope.
    /// `opener` is relative to this node, independent of requester roles for wanted work.
    /// The initial queue can contain a locally requested opening control.
    /// # Errors
    /// Denied, unavailable or exhausted local capabilities refuse only this stream.
    fn open(
        &self,
        scope: Scope,
        opener: Direction,
        output: &mut Output,
    ) -> Result<Box<dyn Handler>, Error>;
}
/// Finite framed output owned by one stream; END is admitted only after these bytes drain.
pub struct Output {
    pub(super) queue: Outbound,
    pub(super) end: bool,
    pub(super) received_at: std::time::Instant,
    pub(super) drained_at: Option<std::time::Instant>,
}
impl Output {
    /// Observe when Transport supplied the bytes completing the current received record, before worker queue delay.
    pub fn received_at(&self) -> std::time::Instant {
        tracing::trace!("Reading the original authenticated record receipt time");
        self.received_at
    }
    /// Observe the latest complete local output drain into Transport, rather than a later worker callback time.
    pub fn drained_at(&self) -> Option<std::time::Instant> {
        tracing::trace!("Reading the original local Transport admission observation");
        self.drained_at
    }
    /// Borrow immutable queue state for producer timing and progress; this grants no acknowledgement mutation.
    pub fn queue(&self) -> &Outbound {
        tracing::trace!("Borrowing immutable peer output admission state");
        &self.queue
    }
    /// Retain a complete valid record before END, without exceeding the stream byte budget.
    /// # Errors
    /// Invalid, Conflict or Limit leaves previously admitted records unchanged.
    pub fn enqueue(&mut self, record: &Record) -> Result<(), Error> {
        tracing::trace!("Queuing a handler-owned framed peer record");
        if self.end {
            return Err(Error::Conflict);
        }
        self.queue.enqueue(record)
    }
    /// Request local END after all already queued records; this is not a delivery acknowledgement.
    pub fn end(&mut self) {
        tracing::trace!("Requesting peer END after queued records");
        self.end = true;
    }
    /// Inspect remaining framed-byte capacity before constructing output bodies.
    pub fn available(&self) -> usize {
        tracing::trace!("Inspecting handler output capacity");
        self.queue.available()
    }
}
/// Local scheduling limits, independent of normative wire maxima.
#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// Maximum authenticated connection observations retained.
    pub connections: usize,
    /// Maximum streams, including those awaiting late dependency cleanup.
    pub exchanges: usize,
    /// Fixed independent workers, at most64 and no greater than exchanges.
    pub workers: usize,
    /// Maximum pending input bytes per stream, excluding the bounded record decoder.
    pub input_bytes: usize,
    /// Maximum queued output bytes per stream; must fit a maximum control record.
    pub output_bytes: usize,
    /// Aggregate reservation for adapter framing, input and output ownership.
    pub memory_bytes: usize,
    /// Maximum controls per stream per one-second monotonic window.
    pub controls_per_second: usize,
    /// Maximum stream lifetime, measured from admission rather than peer clocks.
    pub lifetime: Duration,
}
impl Config {
    // Reserve worst-case decoder/record handoff copies, queued input and encoded output ownership.
    pub(super) fn reservation(&self) -> Result<usize, Error> {
        tracing::trace!("Validating aggregate peer adapter memory reservations");
        if self.connections == 0
            || self.exchanges == 0
            || self.exchanges > 4096
            || self.workers == 0
            || self.workers > 64
            || self.workers > self.exchanges
            || self.input_bytes == 0
            || self.output_bytes < CONTROL_MAX + 5
            || self.controls_per_second == 0
            || self.lifetime.is_zero()
            || std::time::Instant::now()
                .checked_add(self.lifetime)
                .is_none()
        {
            return Err(Error::Invalid);
        }
        let reservation = (CONTROL_MAX + 17)
            .checked_mul(2)
            .and_then(|n| n.checked_add(self.input_bytes))
            .and_then(|n| {
                self.output_bytes
                    .checked_mul(2)
                    .and_then(|out| n.checked_add(out))
            })
            .ok_or(Error::Invalid)?;
        if reservation > self.memory_bytes {
            return Err(Error::Invalid);
        }
        Ok(reservation)
    }
}
