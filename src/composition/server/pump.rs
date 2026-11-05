//! Bounded driver polling and race-safe task notifications, independent of OS signal handling.
use super::Failure;
use crate::modules::transport::api::{Event, TransportControl};
use std::{
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
    time::Duration,
};
/// Maximum observations handled per turn before returning control to application lifecycle work.
pub const EVENTS_PER_TURN: usize = 64;
struct State {
    pending: Mutex<bool>,
    ready: Condvar,
    shutdown: AtomicBool,
    force: AtomicBool,
}
/// Cloneable local wake/shutdown authority; no peer bytes can construct runtime control requests.
#[derive(Clone)]
pub struct Signal(Arc<State>);
impl Default for Signal {
    /// Create an idle notification source with no shutdown request.
    fn default() -> Self {
        tracing::trace!("Creating server task notification state");
        Self(Arc::new(State {
            pending: Mutex::new(false),
            ready: Condvar::new(),
            shutdown: AtomicBool::new(false),
            force: AtomicBool::new(false),
        }))
    }
}
impl Signal {
    /// Request controlled shutdown and interrupt an idle runtime wait.
    pub fn shutdown(&self) {
        tracing::trace!("Requesting server shutdown through local control");
        if self.0.shutdown.swap(true, Ordering::SeqCst) {
            self.0.force.store(true, Ordering::SeqCst);
        }
        self.notify();
    }
    /// Read whether repeated shutdown requested immediate forced closure.
    pub fn forcing(&self) -> bool {
        tracing::trace!("Checking forced shutdown state");
        self.0.force.load(Ordering::SeqCst)
    }
    /// Read whether local shutdown was requested without consuming the request.
    pub fn stopping(&self) -> bool {
        tracing::trace!("Checking server shutdown state");
        self.0.shutdown.load(Ordering::SeqCst)
    }
    /// Wake a pending poll; notifications delivered before wait remain recorded.
    pub fn notify(&self) {
        tracing::trace!("Notifying server event polling");
        let mut pending = self.0.pending.lock().unwrap_or_else(|e| e.into_inner());
        *pending = true;
        self.0.ready.notify_one();
    }
    /// Wait for notification or a finite lifecycle timeout without losing an earlier wake.
    /// # Errors
    /// Poisoned synchronization returns Runtime instead of panicking.
    pub fn wait(&self, timeout: Duration) -> Result<(), Failure> {
        tracing::trace!("Waiting for server readiness or lifecycle deadline");
        let pending = self.0.pending.lock().map_err(|_| Failure::Runtime)?;
        let (mut pending, _) = self
            .0
            .ready
            .wait_timeout_while(pending, timeout, |pending| !*pending && !self.stopping())
            .map_err(|_| Failure::Runtime)?;
        *pending = false;
        Ok(())
    }
}
impl Wake for State {
    // Convert an owned readiness wake into the same persistent notification as a borrowed wake.
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    // Record readiness before notifying; the runtime may not yet have entered its wait.
    fn wake_by_ref(self: &Arc<Self>) {
        Signal(self.clone()).notify();
    }
}
/// One trusted owner polls the driver and routes events outside protocol internals.
pub struct Pump<T> {
    /// Owned local Transport control boundary.
    pub transport: T,
    /// Cloneable wake and shutdown authority for application composition.
    pub signal: Signal,
}
impl<T: TransportControl> Pump<T> {
    /// Own a driver and a notification source without polling or binding sockets.
    pub fn new(transport: T, signal: Signal) -> Self {
        tracing::trace!("Composing bounded server event polling");
        Self { transport, signal }
    }
    /// Dispatch at most EVENTS_PER_TURN observations and return the count.
    /// A full turn requires another turn before waiting; zero means pending with wakeups registered.
    /// # Errors
    /// Propagates consumer failure and rejects unexpected driver termination.
    pub fn turn(
        &mut self,
        mut consume: impl FnMut(Event) -> Result<(), Failure>,
    ) -> Result<usize, Failure> {
        self.turn_with(|_, event| consume(event))
    }
    /// Poll a bounded turn while giving trusted routing access to its owned control boundary.
    /// # Errors
    /// Propagates consumer failure and unexpected driver termination.
    pub fn turn_with(
        &mut self,
        mut consume: impl FnMut(&mut T, Event) -> Result<(), Failure>,
    ) -> Result<usize, Failure> {
        tracing::trace!("Polling a bounded server event turn");
        let waker = Waker::from(self.signal.0.clone());
        let mut context = Context::from_waker(&waker);
        for count in 0..EVENTS_PER_TURN {
            match self.transport.poll_event(&mut context) {
                Poll::Pending => return Ok(count),
                Poll::Ready(None) => return Err(Failure::Runtime),
                Poll::Ready(Some(event)) => consume(&mut self.transport, event)?,
            }
        }
        Ok(EVENTS_PER_TURN)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::transport::{
        TransportError,
        api::{Accepted, Request},
        handles::ConnectionHandle,
    };
    struct Driver {
        remaining: usize,
        ended: bool,
    }
    impl TransportControl for Driver {
        // Test control rejects requests; only event scheduling is under test.
        fn submit(&mut self, _: Request) -> Result<Accepted, TransportError> {
            Err(TransportError::InvalidRequest)
        }
        // Produce bounded closure observations, then register a wake before reporting pending.
        fn poll_event(&mut self, cx: &mut Context<'_>) -> Poll<Option<Event>> {
            if self.ended {
                return Poll::Ready(None);
            }
            if self.remaining == 0 {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
            self.remaining -= 1;
            Poll::Ready(Some(Event::ConnectionClosed {
                connection: ConnectionHandle::new().id().clone(),
                failure: None,
            }))
        }
    }
    // Wake-before-wait, repeated notifications and shutdown remain observable without a timing-dependent sleep.
    #[test]
    fn notifications_and_shutdown_are_retained() {
        let signal = Signal::default();
        signal.notify();
        signal.wait(Duration::ZERO).unwrap();
        signal.shutdown();
        assert!(signal.stopping());
        assert!(!signal.forcing());
        signal.shutdown();
        assert!(signal.forcing());
        signal.wait(Duration::ZERO).unwrap();
    }
    // A busy driver yields after the finite event budget; pending, consumer failure and terminal driver are distinct.
    #[test]
    fn polling_is_bounded_and_propagates_failures() {
        let mut pump = Pump::new(
            Driver {
                remaining: 65,
                ended: false,
            },
            Signal::default(),
        );
        let mut observed = 0;
        assert_eq!(
            pump.turn(|_| {
                observed += 1;
                Ok(())
            }),
            Ok(64)
        );
        assert_eq!(
            pump.turn(|_| {
                observed += 1;
                Ok(())
            }),
            Ok(1)
        );
        assert_eq!(observed, 65);
        pump.signal.wait(Duration::ZERO).unwrap();
        assert_eq!(pump.turn(|_| Ok(())), Ok(0));
        pump.transport.remaining = 1;
        assert_eq!(pump.turn(|_| Err(Failure::Runtime)), Err(Failure::Runtime));
        pump.transport.ended = true;
        assert_eq!(pump.turn(|_| Ok(())), Err(Failure::Runtime));
    }
}
