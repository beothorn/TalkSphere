//! Monotonic clock wakeups sharing the adapter readiness worker.
use super::reactor::Reactor;
use crate::modules::transport::ports::io::Clock;
use std::{
    io,
    task::{Context, Poll},
    time::Instant,
};

/// One driver timer registration; Drop cancels its retained waker.
pub struct SystemClock {
    reactor: Reactor,
    token: usize,
}
impl SystemClock {
    /// Create an independent monotonic timer worker.
    /// # Errors
    /// Returns OS readiness-worker initialization or thread-spawn failures.
    pub fn new() -> io::Result<Self> {
        tracing::trace!(
            function = "SystemClock::new",
            "Creating the monotonic driver clock"
        );
        Self::shared(Reactor::new()?)
    }
    // Share a provider's worker while keeping this clock's deadline registration separate.
    pub(super) fn shared(reactor: Reactor) -> io::Result<Self> {
        tracing::trace!(
            function = "SystemClock::shared",
            "Registering a shared monotonic timer"
        );
        let token = reactor.token()?;
        Ok(Self { reactor, token })
    }
}
impl Clock for SystemClock {
    /// Read process-local monotonic time; never emits wall-clock or wire timestamps.
    fn now(&self) -> Instant {
        tracing::trace!(function = "SystemClock::now", "Reading monotonic time");
        Instant::now()
    }
    /// Replace the pending deadline and arrange a task wakeup when it becomes due.
    fn poll_deadline(&mut self, cx: &mut Context<'_>, deadline: Instant) -> Poll<()> {
        tracing::trace!(
            function = "SystemClock::poll_deadline",
            "Polling the driver's deadline"
        );
        if self.now() >= deadline {
            return Poll::Ready(());
        }
        self.reactor.timer(self.token, deadline, cx.waker());
        Poll::Pending
    }
}
impl Drop for SystemClock {
    // Remove the timer without waiting for its deadline.
    fn drop(&mut self) {
        tracing::trace!(
            function = "adapter::drop",
            "Releasing an owned platform adapter"
        );
        self.reactor.remove(self.token);
    }
}
