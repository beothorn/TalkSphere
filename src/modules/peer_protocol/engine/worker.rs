//! Finite asynchronous ownership transfer for synchronous module dependency calls.
//! Composition submits each operation once and consumes its correlated completion,
//! including late failures; abandoning a caller never establishes financial rollback.
use super::super::Error;
use std::{
    collections::BTreeSet,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex, mpsc},
    task::{Context, Poll, Waker},
    thread,
};

/// One owned dependency operation; its result and captured state need separate byte budgets.
pub type Job<T> = Box<dyn FnOnce() -> Result<T, Error> + Send + 'static>;
/// Refused admission returns ownership of the operation without executing it.
pub type Rejected<T> = (Error, Job<T>);
/// Process-local correlation; distinct even after an earlier completion is consumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ticket(u64);
/// One completed dependency call; Unknown can include committed owner effects.
pub struct Completion<T> {
    /// Original admission identifier, never inferred from a payload or result.
    pub ticket: Ticket,
    /// Exact module result; worker panics conservatively become Unknown.
    pub result: Result<T, Error>,
}
struct Work<T> {
    ticket: Ticket,
    job: Job<T>,
}
#[derive(Default)]
struct Signal(Mutex<Option<Waker>>);
impl Signal {
    // Publish the reactor waker before examining completions, avoiding a lost wakeup.
    fn register(&self, waker: &Waker) {
        tracing::trace!("Registering the dependency completion observer");
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(waker.clone());
    }
    // Invoke outside the mutex so a synchronous wake implementation cannot deadlock registration.
    fn notify(&self) {
        tracing::trace!("Waking the dependency completion observer");
        let waker = self.0.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}
/// Fixed workers and bounded outstanding calls, including unconsumed completions.
///
/// Submission and polling never execute dependency code on the caller's thread.
/// Accepted jobs are not cancelled or retried on expiry: their durable owner must
/// resolve effects. Call `stop` and consume all outstanding completions before
/// normal shutdown. Dropping detaches workers; already accepted work can finish.
/// Workers must eventually return; a blocked dependency cannot be forcibly killed.
/// This pool bounds counts, not the size of job captures or results.
pub struct Pool<T> {
    sender: Option<mpsc::SyncSender<Work<T>>>,
    receiver: mpsc::Receiver<Completion<T>>,
    signal: Arc<Signal>,
    outstanding: BTreeSet<Ticket>,
    next: u64,
    capacity: usize,
}
impl<T: Send + 'static> Pool<T> {
    /// Start a finite number of workers with an aggregate outstanding-call budget.
    /// `threads` is in1..=64 and no greater than `capacity`, which is at most4096.
    /// # Errors
    /// Invalid rejects impossible limits; Unavailable reports OS thread creation failure.
    pub fn new(threads: usize, capacity: usize) -> Result<Self, Error> {
        tracing::trace!("Starting bounded Peer Protocol dependency workers");
        if threads == 0 || threads > 64 || threads > capacity || capacity > 4096 {
            return Err(Error::Invalid);
        }
        let (sender, jobs) = mpsc::sync_channel::<Work<T>>(capacity);
        let jobs = Arc::new(Mutex::new(jobs));
        let (replies, receiver) = mpsc::sync_channel(capacity);
        let signal = Arc::new(Signal::default());
        for index in 0..threads {
            let jobs = jobs.clone();
            let replies = replies.clone();
            let signal = signal.clone();
            thread::Builder::new()
                .name(format!("peer-dependency-{index}"))
                .spawn(move || run(jobs, replies, signal))
                .map_err(|_| Error::Unavailable)?;
        }
        Ok(Self {
            sender: Some(sender),
            receiver,
            signal,
            outstanding: BTreeSet::new(),
            next: 0,
            capacity,
        })
    }
    /// Transfer one operation exactly once without waiting for a worker or queue slot.
    /// # Errors
    /// Limit, Unavailable or exhausted correlation space returns the unexecuted job.
    /// A successful ticket means accepted work, not completed or committed work.
    pub fn submit(&mut self, job: Job<T>) -> Result<Ticket, Rejected<T>> {
        tracing::trace!("Admitting a bounded synchronous dependency operation");
        let Some(sender) = &self.sender else {
            return Err((Error::Unavailable, job));
        };
        if self.outstanding.len() >= self.capacity {
            return Err((Error::Limit, job));
        }
        let Some(next) = self.next.checked_add(1) else {
            return Err((Error::Limit, job));
        };
        let ticket = Ticket(next);
        match sender.try_send(Work { ticket, job }) {
            Ok(()) => {
                self.next = next;
                self.outstanding.insert(ticket);
                Ok(ticket)
            }
            Err(mpsc::TrySendError::Full(work)) => Err((Error::Limit, work.job)),
            Err(mpsc::TrySendError::Disconnected(work)) => Err((Error::Unavailable, work.job)),
        }
    }
    /// Observe one completion without blocking; registering first prevents lost wakeups.
    /// # Errors
    /// Unavailable means all workers disconnected with unresolved work; it never proves rollback.
    /// Conflict indicates an invalid internal correlation, never permission to retry.
    pub fn poll(&mut self, context: &mut Context<'_>) -> Poll<Result<Completion<T>, Error>> {
        tracing::trace!("Polling a correlated dependency completion");
        self.signal.register(context.waker());
        match self.receiver.try_recv() {
            Ok(completion) if self.outstanding.remove(&completion.ticket) => {
                Poll::Ready(Ok(completion))
            }
            Ok(_) => Poll::Ready(Err(Error::Conflict)),
            Err(mpsc::TryRecvError::Empty) => Poll::Pending,
            Err(mpsc::TryRecvError::Disconnected) => Poll::Ready(Err(Error::Unavailable)),
        }
    }
    /// Stop new admission while allowing accepted work and its completions to finish.
    pub fn stop(&mut self) {
        tracing::trace!("Stopping new dependency admission without abandoning accepted effects");
        self.sender.take();
    }
    /// Count accepted operations whose completion has not yet been consumed.
    pub fn outstanding(&self) -> usize {
        tracing::trace!("Inspecting bounded outstanding dependency work");
        self.outstanding.len()
    }
}
// Serialize only receipt of jobs; release the mutex before any dependency callback runs.
fn run<T: Send + 'static>(
    jobs: Arc<Mutex<mpsc::Receiver<Work<T>>>>,
    replies: mpsc::SyncSender<Completion<T>>,
    signal: Arc<Signal>,
) {
    tracing::trace!("Running Peer Protocol dependency work outside the reactor");
    loop {
        let work = jobs.lock().unwrap_or_else(|e| e.into_inner()).recv();
        let Ok(work) = work else { break };
        let result = catch_unwind(AssertUnwindSafe(work.job)).unwrap_or(Err(Error::Unknown));
        if replies
            .send(Completion {
                ticket: work.ticket,
                result,
            })
            .is_err()
        {
            break;
        }
        signal.notify();
    }
}
#[cfg(test)]
#[path = "worker_tests.rs"]
mod tests;
