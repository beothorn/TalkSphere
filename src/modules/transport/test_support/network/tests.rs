use super::*;
use crate::modules::transport::test_support::{socket::ScriptedSocket, wake::WakeCounter};

fn ready(poll: Poll<SocketResult>) -> SocketResult {
    match poll {
        Poll::Ready(result) => result,
        Poll::Pending => panic!("expected ready socket result"),
    }
}

#[test]
fn connect_completion_wakes_once_and_transfers_socket_ownership() {
    let (mut attempt, completion) = ScriptedConnecting::new();
    let counter = Arc::new(WakeCounter::default());
    let waker = counter.waker();
    let mut cx = Context::from_waker(&waker);
    assert!(attempt.poll_connected(&mut cx).is_pending());
    let (socket, socket_control) = ScriptedSocket::new();
    completion.push(Ok(Box::new(socket)));
    assert_eq!(counter.count(), 1);
    let connected = ready(attempt.poll_connected(&mut cx)).unwrap();
    assert!(completion.is_closed());
    assert!(
        matches!(ready(attempt.poll_connected(&mut cx)), Err(error) if error.kind() == io::ErrorKind::NotConnected)
    );
    drop(attempt);
    assert!(!socket_control.is_closed());
    drop(connected);
    assert!(socket_control.is_closed());
}

#[test]
fn connection_failure_and_cancelled_attempts_release_resources() {
    let (mut attempt, completion) = ScriptedConnecting::new();
    completion.push(Err(io::ErrorKind::ConnectionRefused.into()));
    assert!(
        matches!(ready(attempt.poll_connected(&mut Context::from_waker(Waker::noop()))), Err(error) if error.kind() == io::ErrorKind::ConnectionRefused)
    );
    let (attempt, completion) = ScriptedConnecting::new();
    let (socket, socket_control) = ScriptedSocket::new();
    completion.push(Ok(Box::new(socket)));
    drop(attempt);
    assert!(completion.is_closed());
    assert!(socket_control.is_closed());
}

#[test]
fn listener_preserves_accept_order_and_close_releases_queued_sockets() {
    let address = "127.0.0.1:9876".parse().unwrap();
    let (mut listener, control) = ScriptedListener::new(address);
    let counter = Arc::new(WakeCounter::default());
    let waker = counter.waker();
    let mut cx = Context::from_waker(&waker);
    assert_eq!(listener.local_addr().unwrap(), address);
    assert!(listener.poll_accept(&mut cx).is_pending());
    let (socket, first) = ScriptedSocket::new();
    control.push(Ok(Box::new(socket)));
    control.push(Err(io::ErrorKind::ConnectionAborted.into()));
    assert_eq!(counter.count(), 1);
    let accepted = ready(listener.poll_accept(&mut cx)).unwrap();
    assert!(
        matches!(ready(listener.poll_accept(&mut cx)), Err(error) if error.kind() == io::ErrorKind::ConnectionAborted)
    );
    let (socket, queued) = ScriptedSocket::new();
    control.push(Ok(Box::new(socket)));
    listener.close().unwrap();
    assert!(queued.is_closed());
    assert!(!first.is_closed());
    assert!(control.is_closed());
    assert!(
        matches!(ready(listener.poll_accept(&mut cx)), Err(error) if error.kind() == io::ErrorKind::NotConnected)
    );
    drop(accepted);
    assert!(first.is_closed());
}

#[test]
fn dropping_a_pending_listener_wakes_its_waiter() {
    let (mut listener, control) = ScriptedListener::new("127.0.0.1:9876".parse().unwrap());
    let counter = Arc::new(WakeCounter::default());
    assert!(
        listener
            .poll_accept(&mut Context::from_waker(&counter.waker()))
            .is_pending()
    );
    drop(listener);
    assert!(control.is_closed());
    assert_eq!(counter.count(), 1);
}

#[test]
fn provider_records_endpoints_and_preserves_immediate_errors() {
    let endpoint = "127.0.0.1:9000".parse().unwrap();
    let bind = "127.0.0.1:0".parse().unwrap();
    let (attempt, _) = ScriptedConnecting::new();
    let (listener, _) = ScriptedListener::new(endpoint);
    let calls = vec![
        NetworkCall::Connect(endpoint),
        NetworkCall::Listen(bind),
        NetworkCall::Connect(endpoint),
        NetworkCall::Listen(bind),
    ];
    let script = Script::new(calls.iter().cloned().zip([
        NetworkReply::Connect(Ok(Box::new(attempt))),
        NetworkReply::Listen(Ok(Box::new(listener))),
        NetworkReply::Connect(Err(io::ErrorKind::PermissionDenied.into())),
        NetworkReply::Listen(Err(io::ErrorKind::AddrInUse.into())),
    ]));
    let mut network = ScriptedNetwork(script.clone());
    let _ = network.connect(endpoint).unwrap();
    assert_eq!(
        network.listen(bind).unwrap().local_addr().unwrap(),
        endpoint
    );
    assert!(
        matches!(network.connect(endpoint), Err(error) if error.kind() == io::ErrorKind::PermissionDenied)
    );
    assert!(matches!(network.listen(bind), Err(error) if error.kind() == io::ErrorKind::AddrInUse));
    assert_eq!(script.calls(), calls);
    script.assert_finished();
}

#[test]
#[should_panic(expected = "already has a completion")]
fn connection_attempts_reject_multiple_scripted_completions() {
    let (_attempt, control) = ScriptedConnecting::new();
    control.push(Err(io::ErrorKind::ConnectionRefused.into()));
    control.push(Err(io::ErrorKind::TimedOut.into()));
}
