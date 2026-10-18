//! Real loopback readiness checks; no cryptographic substitute is exposed by adapters.
use super::*;
use crate::modules::transport::ports::io::{Clock, SocketProvider};
use std::{
    sync::Arc,
    task::{Context, Poll, Wake},
    time::{Duration, Instant},
};
struct ThreadWake(std::thread::Thread);
impl Wake for ThreadWake {
    // Wake the polling test thread; park tokens avoid lost notifications before parking.
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}
// Bound real OS waits while responding only to readiness or timer wakeups.
fn wait<T>(mut poll: impl FnMut(&mut Context<'_>) -> Poll<T>) -> T {
    let waker = Arc::new(ThreadWake(std::thread::current())).into();
    let mut cx = Context::from_waker(&waker);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Poll::Ready(value) = poll(&mut cx) {
            return value;
        }
        assert!(Instant::now() < deadline, "adapter failed to wake");
        std::thread::park_timeout(deadline.saturating_duration_since(Instant::now()));
    }
}

// OS-assigned loopback ports support connect, accept, partial payload I/O, EOF and explicit close.
#[test]
fn loopback_stream_and_timer_wakeups() {
    let mut provider = TcpProvider::new().unwrap();
    let mut listener = provider.listen("127.0.0.1:0".parse().unwrap()).unwrap();
    let address = listener.local_addr().unwrap();
    assert_ne!(address.port(), 0);
    let mut attempt = provider.connect(address).unwrap();
    let mut client = wait(|cx| attempt.poll_connected(cx)).unwrap();
    let mut server = wait(|cx| listener.poll_accept(cx)).unwrap();
    let mut buffer = [0; 8];
    assert!(
        server
            .poll_read(
                &mut Context::from_waker(std::task::Waker::noop()),
                &mut buffer
            )
            .is_pending()
    );
    assert_eq!(wait(|cx| client.poll_write(cx, b"hello")).unwrap(), 5);
    assert_eq!(wait(|cx| server.poll_read(cx, &mut buffer)).unwrap(), 5);
    assert_eq!(&buffer[..5], b"hello");
    client.close().unwrap();
    assert_eq!(wait(|cx| server.poll_read(cx, &mut buffer)).unwrap(), 0);
    listener.close().unwrap();
    assert!(listener.local_addr().is_err());
    let mut clock = provider.clock().unwrap();
    let deadline = clock.now() + Duration::from_millis(5);
    wait(|cx| clock.poll_deadline(cx, deadline));
    assert!(clock.now() >= deadline);
}

// Bind conflicts and refused connections remain errors rather than false successful admission.
#[test]
fn bind_conflict_and_connect_refusal() {
    let mut provider = TcpProvider::new().unwrap();
    let listener = provider.listen("127.0.0.1:0".parse().unwrap()).unwrap();
    let address = listener.local_addr().unwrap();
    assert!(provider.listen(address).is_err());
    drop(listener);
    if let Ok(mut attempt) = provider.connect(address) {
        assert!(wait(|cx| attempt.poll_connected(cx)).is_err());
    }
    let mut clock = SystemClock::new().unwrap();
    assert!(
        clock
            .poll_deadline(
                &mut Context::from_waker(std::task::Waker::noop()),
                clock.now()
            )
            .is_ready()
    );
}
