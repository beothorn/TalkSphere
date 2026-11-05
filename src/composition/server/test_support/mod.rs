//! Deterministic runtime wake and shutdown probes, unavailable in production builds.
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::task::{Wake, Waker};
#[derive(Default)]
pub(super) struct Control {
    wakes: AtomicUsize,
    stopped: AtomicBool,
}
impl Control {
    // Construct a task waker sharing the fixture's observed wake count.
    pub(super) fn waker(self: &Arc<Self>) -> Waker {
        Waker::from(self.clone())
    }
    // Count wake deliveries without introducing scheduling or timing assumptions.
    pub(super) fn wakes(&self) -> usize {
        self.wakes.load(Ordering::SeqCst)
    }
    // Request shutdown through a test-owned signal rather than a process-global handler.
    pub(super) fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
    }
    // Observe deterministic shutdown state without consuming it.
    pub(super) fn stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }
}
impl Wake for Control {
    // Consuming a waker still records the delivery in the shared fixture.
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    // Record each notification, including notifications delivered before the next poll.
    fn wake_by_ref(self: &Arc<Self>) {
        self.wakes.fetch_add(1, Ordering::SeqCst);
    }
}
// Wake notifications and shutdown requests remain observable across cloned fixture handles.
#[test]
fn wake_and_shutdown_probes_are_deterministic() {
    let control = Arc::new(Control::default());
    let waker = control.waker();
    assert!(!control.stopped());
    assert_eq!(control.wakes(), 0);
    waker.wake_by_ref();
    waker.wake();
    assert_eq!(control.wakes(), 2);
    control.stop();
    control.stop();
    assert!(control.stopped());
}
