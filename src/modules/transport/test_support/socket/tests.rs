use super::*;
use crate::modules::transport::test_support::{vectors, wake::WakeCounter};

fn ready<T>(poll: Poll<io::Result<T>>) -> io::Result<T> {
    match poll {
        Poll::Ready(result) => result,
        Poll::Pending => panic!("expected ready I/O"),
    }
}

#[test]
fn every_fragmentation_and_eof_offset_preserves_exact_fixture_prefix() {
    let bytes = vectors::bytes("complete bytes written to TCP", 0);
    let waker = Waker::noop();
    let mut cx = Context::from_waker(waker);
    for end in 0..=bytes.len() {
        for chunk in 1..=bytes.len() {
            let (mut socket, control) = ScriptedSocket::new();
            for part in bytes[..end].chunks(chunk) {
                control.push_read(ReadStep::Bytes(part.to_vec()));
            }
            control.push_read(ReadStep::Eof);
            let mut received = Vec::new();
            let mut buffer = [0; 7];
            loop {
                let count = ready(socket.poll_read(&mut cx, &mut buffer)).unwrap();
                if count == 0 {
                    break;
                }
                received.extend_from_slice(&buffer[..count]);
            }
            assert_eq!(received, bytes[..end], "end={end} chunk={chunk}");
            assert_eq!(ready(socket.poll_read(&mut cx, &mut buffer)).unwrap(), 0);
            control.assert_finished();
        }
    }
}

#[test]
fn multiple_frames_can_arrive_in_a_single_read() {
    let expected = [
        vectors::bytes("complete bytes written to TCP", 0),
        vectors::bytes("complete bytes written to TCP", 1),
    ]
    .concat();
    let (mut socket, control) = ScriptedSocket::new();
    control.push_read(ReadStep::Bytes(expected.clone()));
    let mut buffer = vec![0; expected.len()];
    let mut cx = Context::from_waker(Waker::noop());
    assert_eq!(
        ready(socket.poll_read(&mut cx, &mut buffer)).unwrap(),
        expected.len()
    );
    assert_eq!(buffer, expected);
    assert!(socket.poll_read(&mut cx, &mut buffer).is_pending());
    control.assert_finished();
}

#[test]
fn short_writes_record_only_accepted_bytes_and_preserve_failures() {
    let (mut socket, control) = ScriptedSocket::new();
    for step in [
        WriteStep::Accept(2),
        WriteStep::Error(io::ErrorKind::BrokenPipe),
        WriteStep::Accept(0),
        WriteStep::Accept(10),
    ] {
        control.push_write(step);
    }
    let mut cx = Context::from_waker(Waker::noop());
    assert_eq!(ready(socket.poll_write(&mut cx, b"abcd")).unwrap(), 2);
    assert_eq!(
        ready(socket.poll_write(&mut cx, b"cd")).unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(ready(socket.poll_write(&mut cx, b"cd")).unwrap(), 0);
    assert_eq!(control.written(), b"ab");
    assert_eq!(ready(socket.poll_write(&mut cx, b"cd")).unwrap(), 2);
    assert_eq!(control.written(), b"abcd");
    control.assert_finished();
}

#[test]
fn pending_directions_are_woken_only_by_their_own_readiness() {
    let (mut socket, control) = ScriptedSocket::new();
    let read = Arc::new(WakeCounter::default());
    let write = Arc::new(WakeCounter::default());
    let rw = read.waker();
    let ww = write.waker();
    let mut rcx = Context::from_waker(&rw);
    let mut wcx = Context::from_waker(&ww);
    assert!(socket.poll_read(&mut rcx, &mut [0; 1]).is_pending());
    assert!(socket.poll_write(&mut wcx, b"x").is_pending());
    control.push_read(ReadStep::Error(io::ErrorKind::ConnectionReset));
    assert_eq!((read.count(), write.count()), (1, 0));
    assert_eq!(
        ready(socket.poll_read(&mut rcx, &mut [0; 1]))
            .unwrap_err()
            .kind(),
        io::ErrorKind::ConnectionReset
    );
    control.push_write(WriteStep::Accept(1));
    assert_eq!((read.count(), write.count()), (1, 1));
    assert_eq!(ready(socket.poll_write(&mut wcx, b"x")).unwrap(), 1);
    control.assert_finished();
}

#[test]
fn empty_buffers_do_not_consume_scripts_and_close_wakes_pending_io() {
    let (mut socket, control) = ScriptedSocket::new();
    control.push_read(ReadStep::Bytes(vec![7]));
    control.push_write(WriteStep::Accept(1));
    let counter = Arc::new(WakeCounter::default());
    let waker = counter.waker();
    let mut cx = Context::from_waker(&waker);
    assert_eq!(ready(socket.poll_read(&mut cx, &mut [])).unwrap(), 0);
    assert_eq!(ready(socket.poll_write(&mut cx, &[])).unwrap(), 0);
    let mut bytes = [0];
    assert_eq!(ready(socket.poll_read(&mut cx, &mut bytes)).unwrap(), 1);
    assert_eq!(bytes, [7]);
    assert_eq!(ready(socket.poll_write(&mut cx, b"x")).unwrap(), 1);
    assert!(socket.poll_read(&mut cx, &mut bytes).is_pending());
    assert!(socket.poll_write(&mut cx, b"x").is_pending());
    socket.close().unwrap();
    socket.close().unwrap();
    assert!(control.is_closed());
    assert_eq!(counter.count(), 2);
    assert_eq!(
        ready(socket.poll_read(&mut cx, &mut bytes))
            .unwrap_err()
            .kind(),
        io::ErrorKind::NotConnected
    );
    assert_eq!(
        ready(socket.poll_write(&mut cx, b"x")).unwrap_err().kind(),
        io::ErrorKind::NotConnected
    );
    control.assert_finished();
}

#[test]
fn dropping_the_socket_closes_it_even_while_a_control_handle_survives() {
    let (socket, control) = ScriptedSocket::new();
    drop(socket);
    assert!(control.is_closed());
}

#[test]
fn a_repoll_replaces_the_waiting_task_waker() {
    let (mut socket, control) = ScriptedSocket::new();
    let first = Arc::new(WakeCounter::default());
    let second = Arc::new(WakeCounter::default());
    assert!(
        socket
            .poll_read(&mut Context::from_waker(&first.waker()), &mut [0])
            .is_pending()
    );
    assert!(
        socket
            .poll_read(&mut Context::from_waker(&second.waker()), &mut [0])
            .is_pending()
    );
    control.push_read(ReadStep::Eof);
    assert_eq!(first.count(), 0);
    assert_eq!(second.count(), 1);
    assert_eq!(
        ready(socket.poll_read(&mut Context::from_waker(Waker::noop()), &mut [0])).unwrap(),
        0
    );
    control.assert_finished();
}

#[test]
#[should_panic(expected = "read added after closure or EOF")]
fn data_after_eof_is_rejected_instead_of_silently_disappearing() {
    let (_socket, control) = ScriptedSocket::new();
    control.push_read(ReadStep::Eof);
    control.push_read(ReadStep::Bytes(vec![1]));
}

#[test]
#[should_panic(expected = "unconsumed socket steps")]
fn unfinished_scripts_report_failure_without_panicking_again_during_cleanup() {
    let (_socket, control) = ScriptedSocket::new();
    control.push_write(WriteStep::Accept(1));
    control.assert_finished();
}
