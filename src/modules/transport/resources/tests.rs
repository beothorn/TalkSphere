//! Shared reservation boundaries, checked arithmetic and ownership release tests.
use super::*;
use crate::modules::transport::{
    codec::{FrameHeader, FrameProtection, FrameType},
    framing::{FrameWriter, WireFrame},
    test_support::socket::{ScriptedSocket, WriteStep},
};

// Shared clones never oversubscribe, and only the final reservation owner releases bytes.
#[test]
fn shared_limits_and_checked_overflow() {
    let resources = Resources::new(ResourceLimits {
        connections: 1,
        streams_per_connection: 1,
        outbound_bytes: 100,
        buffered_bytes_per_stream: 65536,
    });
    let connection = resources.connection().unwrap();
    assert!(resources.clone().connection().is_none());
    assert_eq!(resources.connections(), 1);
    drop(connection);
    assert_eq!(resources.connections(), 0);
    let bytes = resources.outbound(100).unwrap();
    let retained = bytes.retain();
    assert!(resources.outbound(1).is_none());
    assert!(resources.outbound(usize::MAX).is_none());
    drop(bytes);
    assert_eq!(resources.outbound_bytes(), 100);
    drop(retained);
    assert_eq!(resources.outbound_bytes(), 0);
}

// Moving an admitted frame into a partial writer retains its budget until full completion.
#[test]
fn reservation_follows_partial_writer() {
    let resources = Resources::new(ResourceLimits {
        connections: 1,
        streams_per_connection: 1,
        outbound_bytes: 100,
        buffered_bytes_per_stream: 65536,
    });
    let mut frame = WireFrame::new(
        FrameHeader::new(FrameType::Data, 1, 1).unwrap(),
        vec![1],
        Some([0; 16]),
    )
    .unwrap();
    frame.reservation = resources.outbound(29);
    let mut writer = FrameWriter::default();
    writer.start(frame, FrameProtection::Tagged).unwrap();
    assert_eq!(resources.outbound_bytes(), 29);
    let (mut socket, control) = ScriptedSocket::new();
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    control.push_write(WriteStep::Accept(1));
    assert!(writer.poll_flush(&mut socket, &mut cx).is_pending());
    assert_eq!(resources.outbound_bytes(), 29);
    control.push_write(WriteStep::Accept(100));
    assert!(writer.poll_flush(&mut socket, &mut cx).is_ready());
    assert_eq!(resources.outbound_bytes(), 0);
}
