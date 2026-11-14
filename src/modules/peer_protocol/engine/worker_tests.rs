//! Deterministic barriers verify bounded admission, late completion and independent workers.
use super::*;
use std::{sync::mpsc, task::Wake, time::Duration};
struct Notify(mpsc::Sender<()>);
impl Wake for Notify {
    // Wake a blocked test observer without polling or timing assumptions about worker execution.
    fn wake(self: Arc<Self>) {
        let _ = self.0.send(());
    }
}
// Wait for an actual completion wake and fail finitely if a worker or notification is lost.
fn complete<T: Send + 'static>(pool: &mut Pool<T>) -> Completion<T> {
    let (sender, receiver) = mpsc::channel();
    let waker = Waker::from(Arc::new(Notify(sender)));
    let mut context = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(result) = pool.poll(&mut context) {
            return result.unwrap();
        }
        receiver.recv_timeout(Duration::from_secs(5)).unwrap();
    }
}
// Invalid configuration creates no pool; zero capacity and excessive workers cannot be admitted.
#[test]
fn invalid_configuration() {
    for (threads, capacity) in [(0, 1), (1, 0), (2, 1), (65, 65), (1, 4097)] {
        assert!(matches!(
            Pool::<()>::new(threads, capacity),
            Err(Error::Invalid)
        ));
    }
}
// A running or completed-but-unread operation consumes capacity, and rejected work remains owned.
#[test]
fn admission_owns_jobs_once_and_counts_unread_results() {
    let mut pool = Pool::new(1, 1).unwrap();
    let (entered, entry) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let ticket = pool
        .submit(Box::new(move || {
            entered.send(()).unwrap();
            gate.recv().unwrap();
            Ok(7)
        }))
        .unwrap_or_else(|_| panic!("admission failed"));
    entry.recv_timeout(Duration::from_secs(5)).unwrap();
    let (error, job) = pool.submit(Box::new(|| Ok(9))).err().unwrap();
    assert_eq!(error, Error::Limit);
    assert_eq!(job(), Ok(9));
    assert_eq!(pool.outstanding(), 1);
    release.send(()).unwrap();
    let completion = complete(&mut pool);
    assert_eq!(completion.ticket, ticket);
    assert_eq!(completion.result, Ok(7));
    assert_eq!(pool.outstanding(), 0);
    let next = pool
        .submit(Box::new(|| Err(Error::Storage)))
        .unwrap_or_else(|_| panic!("admission failed"));
    assert_ne!(ticket, next);
    assert_eq!(complete(&mut pool).result, Err(Error::Storage));
}
// Stop refuses new work, but a late accepted completion still exposes its original financial uncertainty.
#[test]
fn stopped_admission_preserves_late_completion() {
    let mut pool = Pool::<()>::new(1, 2).unwrap();
    pool.submit(Box::new(|| Err(Error::Unknown)))
        .unwrap_or_else(|_| panic!("admission failed"));
    pool.stop();
    assert_eq!(
        pool.submit(Box::new(|| Ok(()))).err().unwrap().0,
        Error::Unavailable
    );
    assert_eq!(complete(&mut pool).result, Err(Error::Unknown));
    assert_eq!(pool.outstanding(), 0);
}
// A blocked dependency does not hold the queue mutex or prevent an independent worker from completing.
#[test]
fn workers_make_independent_progress() {
    let mut pool = Pool::new(2, 2).unwrap();
    let (entered, entry) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let first = pool
        .submit(Box::new(move || {
            entered.send(()).unwrap();
            gate.recv().unwrap();
            Ok(1)
        }))
        .unwrap_or_else(|_| panic!("admission failed"));
    entry.recv_timeout(Duration::from_secs(5)).unwrap();
    let second = pool
        .submit(Box::new(|| Ok(2)))
        .unwrap_or_else(|_| panic!("admission failed"));
    let result = complete(&mut pool);
    assert_eq!(result.ticket, second);
    assert_eq!(result.result, Ok(2));
    release.send(()).unwrap();
    assert_eq!(complete(&mut pool).ticket, first);
}
// A panic can follow a durable effect, so it is Unknown and does not stop subsequent queued calls.
#[test]
fn panic_is_unknown_without_retrying_work() {
    let mut pool = Pool::<()>::new(1, 1).unwrap();
    pool.submit(Box::new(|| panic!("test dependency fault")))
        .unwrap_or_else(|_| panic!("admission failed"));
    assert_eq!(complete(&mut pool).result, Err(Error::Unknown));
    pool.submit(Box::new(|| Ok(())))
        .unwrap_or_else(|_| panic!("admission failed"));
    assert_eq!(complete(&mut pool).result, Ok(()));
}
// Exhausted correlation space refuses work before execution and never wraps into a previous ticket.
#[test]
fn correlation_does_not_wrap() {
    let mut pool = Pool::<()>::new(1, 1).unwrap();
    pool.next = u64::MAX;
    assert_eq!(
        pool.submit(Box::new(|| Ok(()))).err().unwrap().0,
        Error::Limit
    );
    assert_eq!(pool.outstanding(), 0);
}
