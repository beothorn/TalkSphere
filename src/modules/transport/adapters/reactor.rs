//! Private Mio readiness and timer worker; task wakers run outside its state lock.
use mio::{Events, Interest, Poll, Registry, Token, event::Source};
use std::{
    collections::BTreeMap,
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::Waker,
    thread::JoinHandle,
    time::Instant,
};

#[derive(Default)]
struct Waiters {
    read: Option<Waker>,
    write: Option<Waker>,
    timer: Option<(Instant, Waker)>,
}
struct State {
    entries: Mutex<BTreeMap<usize, Waiters>>,
    stop: AtomicBool,
    error: Mutex<Option<io::ErrorKind>>,
}
struct Inner {
    registry: Registry,
    state: Arc<State>,
    wake: mio::Waker,
    next: AtomicUsize,
    thread: Option<JoinHandle<()>>,
}
#[derive(Clone)]
pub(super) struct Reactor(Arc<Inner>);

impl Reactor {
    // Start one OS readiness worker; every socket and timer retains its lifetime.
    pub(super) fn new() -> io::Result<Self> {
        tracing::trace!(function = "Reactor::new", "Starting TCP readiness worker");
        let poll = Poll::new()?;
        let registry = poll.registry().try_clone()?;
        let wake = mio::Waker::new(&registry, Token(0))?;
        let state = Arc::new(State {
            entries: Mutex::new(BTreeMap::new()),
            stop: AtomicBool::new(false),
            error: Mutex::new(None),
        });
        let worker_state = state.clone();
        let thread = std::thread::Builder::new()
            .name("transport-readiness".into())
            .spawn(move || run(poll, worker_state))?;
        Ok(Self(Arc::new(Inner {
            registry,
            state,
            wake,
            next: AtomicUsize::new(1),
            thread: Some(thread),
        })))
    }
    // Allocate non-reusable local readiness tokens; zero belongs to the worker interrupter.
    pub(super) fn token(&self) -> io::Result<usize> {
        tracing::trace!(
            function = "Reactor::token",
            "Allocating readiness registration"
        );
        let token = self
            .0
            .next
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| io::Error::other("readiness tokens exhausted"))?;
        self.0
            .state
            .entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(token, Waiters::default());
        Ok(token)
    }
    // Register a live socket before any nonblocking operation or connect completion check.
    pub(super) fn register(
        &self,
        source: &mut impl Source,
        interest: Interest,
    ) -> io::Result<usize> {
        tracing::trace!(
            function = "Reactor::register",
            "Registering nonblocking socket interests"
        );
        let token = self.token()?;
        if let Err(error) = self.0.registry.register(source, Token(token), interest) {
            self.remove(token);
            return Err(error);
        }
        Ok(token)
    }
    // Store the task waker before attempting I/O, eliminating the readiness registration race.
    pub(super) fn arm(&self, token: usize, write: bool, waker: &Waker) -> io::Result<()> {
        tracing::trace!(function = "Reactor::arm", "Arming a socket task wakeup");
        if let Some(error) = *self.0.state.error.lock().unwrap_or_else(|e| e.into_inner()) {
            return Err(error.into());
        }
        let mut entries = self
            .0
            .state
            .entries
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let waiters = entries.get_mut(&token).ok_or(io::ErrorKind::NotConnected)?;
        if write {
            waiters.write = Some(waker.clone());
        } else {
            waiters.read = Some(waker.clone());
        }
        Ok(())
    }
    // Replace one driver's timer and interrupt the worker to recompute its nearest deadline.
    pub(super) fn timer(&self, token: usize, deadline: Instant, waker: &Waker) {
        tracing::trace!(
            function = "Reactor::timer",
            "Registering a monotonic deadline wakeup"
        );
        if let Some(waiters) = self
            .0
            .state
            .entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_mut(&token)
        {
            waiters.timer = Some((deadline, waker.clone()));
        }
        let _ = self.0.wake.wake();
    }
    // Remove a closed socket from OS interests and release its retained wakers.
    pub(super) fn deregister(&self, source: &mut impl Source, token: usize) {
        tracing::trace!(
            function = "Reactor::deregister",
            "Releasing socket readiness registration"
        );
        let _ = self.0.registry.deregister(source);
        self.remove(token);
    }
    // Timer and failed-registration cleanup share the same bounded entry table.
    pub(super) fn remove(&self, token: usize) {
        tracing::trace!(
            function = "Reactor::remove",
            "Releasing readiness task state"
        );
        self.0
            .state
            .entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&token);
    }
}
impl Drop for Inner {
    // The final adapter wakes and joins its worker, leaving no detached background task.
    fn drop(&mut self) {
        tracing::trace!(function = "Reactor::drop", "Stopping the readiness worker");
        self.state.stop.store(true, Ordering::Release);
        let _ = self.wake.wake();
        if let Some(thread) = self.thread.take() {
            // A user waker may drop its final adapter on this worker; never self-join.
            if thread.thread().id() != std::thread::current().id() {
                let _ = thread.join();
            }
        }
    }
}

// Poll OS readiness and monotonic deadlines, tolerating interrupts and spurious readiness.
fn run(mut poll: Poll, state: Arc<State>) {
    tracing::trace!(function = "reactor::run", "Running TCP readiness dispatch");
    let mut events = Events::with_capacity(256);
    while !state.stop.load(Ordering::Acquire) {
        let timeout = state
            .entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .filter_map(|entry| {
                entry
                    .timer
                    .as_ref()
                    .map(|(deadline, _)| deadline.saturating_duration_since(Instant::now()))
            })
            .min();
        let error = match poll.poll(&mut events, timeout) {
            Ok(()) => None,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => Some(error.kind()),
        };
        let mut wake = Vec::new();
        {
            let mut entries = state.entries.lock().unwrap_or_else(|e| e.into_inner());
            for event in &events {
                if let Some(entry) = entries.get_mut(&event.token().0) {
                    if event.is_readable() || event.is_error() || event.is_read_closed() {
                        wake.extend(entry.read.take());
                    }
                    if event.is_writable() || event.is_error() || event.is_write_closed() {
                        wake.extend(entry.write.take());
                    }
                }
            }
            let now = Instant::now();
            for entry in entries.values_mut() {
                if entry
                    .timer
                    .as_ref()
                    .is_some_and(|(deadline, _)| *deadline <= now || error.is_some())
                    && let Some((_, waker)) = entry.timer.take()
                {
                    wake.push(waker);
                }
                if error.is_some() {
                    wake.extend(entry.read.take());
                    wake.extend(entry.write.take());
                }
            }
        }
        if let Some(error) = error {
            *state.error.lock().unwrap_or_else(|e| e.into_inner()) = Some(error);
        }
        for waker in wake {
            waker.wake();
        }
        if error.is_some() {
            break;
        }
    }
}
