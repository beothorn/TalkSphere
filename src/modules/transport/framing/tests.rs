//! TR-02/TR-14 byte-stream tests; framing never implies authentication.
use super::*;
use crate::modules::transport::{
    codec::{FrameHeader, FrameProtection, FrameType},
    test_support::{
        socket::{ReadStep, ScriptedSocket, WriteStep},
        vectors,
    },
};
use std::{
    io::ErrorKind,
    task::{Context, Poll, Waker},
};

// Poll deterministic, ready scripts through the cooperative per-poll work budget.
fn read(
    reader: &mut FrameReader,
    socket: &mut ScriptedSocket,
    phase: FrameProtection,
) -> Result<Option<WireFrame>, FramingError> {
    for _ in 0..2_000 {
        if let Poll::Ready(result) =
            reader.poll_frame(socket, &mut Context::from_waker(Waker::noop()), phase)
        {
            return result;
        }
    }
    panic!("script did not complete");
}
// Create an outgoing fixture without generating its expected bytes locally.
fn fixture() -> WireFrame {
    WireFrame::new(
        FrameHeader::new(FrameType::Ping, 0, 8).unwrap(),
        vectors::bytes("payload", 0),
        Some(vectors::bytes("integrity tag", 0).try_into().unwrap()),
    )
    .unwrap()
}
// Every TCP split and coalescing of two frames must preserve their exact independent bytes.
#[test]
fn all_splits_and_coalesced_frames_preserve_boundaries() {
    let expected = vectors::bytes("complete bytes written to TCP", 0);
    for split in 0..=expected.len() {
        let (mut socket, control) = ScriptedSocket::new();
        for part in [&expected[..split], &expected[split..], expected.as_slice()] {
            if !part.is_empty() {
                control.push_read(ReadStep::Bytes(part.to_vec()));
            }
        }
        control.push_read(ReadStep::Eof);
        let mut reader = FrameReader::default();
        for _ in 0..2 {
            let frame = read(&mut reader, &mut socket, FrameProtection::Tagged)
                .unwrap()
                .unwrap();
            assert_eq!(frame.header().frame_type(), FrameType::Ping);
            assert_eq!(frame.payload(), vectors::bytes("payload", 0));
            assert_eq!(
                frame.tag().unwrap().as_slice(),
                vectors::bytes("integrity tag", 0)
            );
        }
        assert!(
            read(&mut reader, &mut socket, FrameProtection::Tagged)
                .unwrap()
                .is_none()
        );
        assert!(
            read(&mut reader, &mut socket, FrameProtection::Tagged)
                .unwrap()
                .is_none()
        );
        control.assert_finished();
    }
}
// EOF after any nonempty partial prefix is terminal and never exposes provisional payload.
#[test]
fn every_truncation_offset_fails_without_resynchronizing() {
    let bytes = vectors::bytes("complete bytes written to TCP", 0);
    for end in 1..bytes.len() {
        let (mut socket, control) = ScriptedSocket::new();
        control.push_read(ReadStep::Bytes(bytes[..end].to_vec()));
        control.push_read(ReadStep::Eof);
        let mut reader = FrameReader::default();
        assert_eq!(
            read(&mut reader, &mut socket, FrameProtection::Tagged).err(),
            Some(FramingError::Truncated)
        );
        assert_eq!(
            read(&mut reader, &mut socket, FrameProtection::Tagged).err(),
            Some(FramingError::Closed)
        );
    }
}
// Oversized headers fail before reading any claimed payload or subsequent valid frame.
#[test]
fn invalid_headers_stop_before_body_reads() {
    let (mut socket, control) = ScriptedSocket::new();
    control.push_read(ReadStep::Bytes(vec![1, 0x11, 0, 0, 0, 0, 0, 1, 0, 1, 0, 1]));
    let mut reader = FrameReader::default();
    assert_eq!(
        read(&mut reader, &mut socket, FrameProtection::Tagged).err(),
        Some(FramingError::Header(
            crate::modules::transport::codec::HeaderError::PayloadTooLarge(65537)
        ))
    );
    control.assert_finished();
    control.push_read(ReadStep::Bytes(vectors::bytes(
        "complete bytes written to TCP",
        0,
    )));
    assert_eq!(
        read(&mut reader, &mut socket, FrameProtection::Tagged).err(),
        Some(FramingError::Closed)
    );
}
// Pending retains offsets and mode; a mid-frame phase change is a caller error, never guessed.
#[test]
fn pending_reads_preserve_state_and_reject_phase_changes() {
    let (mut socket, control) = ScriptedSocket::new();
    let mut reader = FrameReader::default();
    let mut cx = Context::from_waker(Waker::noop());
    control.push_read(ReadStep::Bytes(vec![1]));
    assert!(
        reader
            .poll_frame(&mut socket, &mut cx, FrameProtection::Untagged)
            .is_pending()
    );
    assert_eq!(
        reader
            .poll_frame(&mut socket, &mut cx, FrameProtection::Tagged)
            .map(|r| r.err()),
        Poll::Ready(Some(FramingError::PhaseChanged))
    );
}
// Short writes cannot mix frames, lose accepted bytes, or restart after Pending.
#[test]
fn partial_writes_are_serialized_and_resume_exactly() {
    let (mut socket, control) = ScriptedSocket::new();
    let mut writer = FrameWriter::default();
    let mut cx = Context::from_waker(Waker::noop());
    writer.start(fixture(), FrameProtection::Tagged).unwrap();
    control.push_write(WriteStep::Accept(1));
    assert!(writer.poll_flush(&mut socket, &mut cx).is_pending());
    assert_eq!(
        writer.start(fixture(), FrameProtection::Tagged),
        Err(FramingError::Busy)
    );
    for _ in 0..35 {
        control.push_write(WriteStep::Accept(1));
    }
    assert_eq!(writer.poll_flush(&mut socket, &mut cx), Poll::Ready(Ok(())));
    assert!(!writer.is_busy());
    assert_eq!(
        control.written(),
        vectors::bytes("complete bytes written to TCP", 0)
    );
    control.assert_finished();
}
// Full-sized data with one-byte operations yields cooperatively and still completes exactly.
#[test]
fn maximum_payload_and_empty_untagged_frames_are_supported() {
    for size in [0, 65_536] {
        let kind = if size == 0 {
            FrameType::End
        } else {
            FrameType::Data
        };
        let header = FrameHeader::new(kind, 1, size).unwrap();
        let expected = [
            header.encode().as_slice(),
            vec![7; size as usize].as_slice(),
        ]
        .concat();
        let (mut socket, control) = ScriptedSocket::new();
        for &byte in &expected {
            control.push_read(ReadStep::Bytes(vec![byte]));
            control.push_write(WriteStep::Accept(1));
        }
        let frame = read(
            &mut FrameReader::default(),
            &mut socket,
            FrameProtection::Untagged,
        )
        .unwrap()
        .unwrap();
        assert!(frame.tag().is_none());
        assert_eq!(frame.payload(), vec![7; size as usize]);
        let mut writer = FrameWriter::default();
        writer.start(frame, FrameProtection::Untagged).unwrap();
        while writer.is_busy() {
            if let Poll::Ready(result) =
                writer.poll_flush(&mut socket, &mut Context::from_waker(Waker::noop()))
            {
                result.unwrap();
            }
        }
        assert_eq!(control.written(), expected);
        control.assert_finished();
    }
}
// Interrupted reads/writes retry; real I/O failures and zero writes permanently stop progress.
#[test]
fn io_failures_and_invalid_local_shapes_are_explicit() {
    let header = FrameHeader::new(FrameType::Data, 1, 1).unwrap();
    assert_eq!(
        WireFrame::new(header, vec![], None).err(),
        Some(FramingError::Shape)
    );
    assert_eq!(
        WireFrame::new(header, vec![9], None)
            .unwrap()
            .into_payload(),
        [9]
    );
    let mut writer = FrameWriter::default();
    assert_eq!(
        writer.start(fixture(), FrameProtection::Untagged),
        Err(FramingError::Shape)
    );
    for failure in [
        WriteStep::Accept(0),
        WriteStep::Error(ErrorKind::BrokenPipe),
    ] {
        let expected = if matches!(failure, WriteStep::Accept(_)) {
            ErrorKind::WriteZero
        } else {
            ErrorKind::BrokenPipe
        };
        let (mut socket, control) = ScriptedSocket::new();
        let mut writer = FrameWriter::default();
        writer.start(fixture(), FrameProtection::Tagged).unwrap();
        control.push_write(WriteStep::Error(ErrorKind::Interrupted));
        control.push_write(failure);
        let mut cx = Context::from_waker(Waker::noop());
        assert_eq!(
            writer.poll_flush(&mut socket, &mut cx),
            Poll::Ready(Err(FramingError::Io(expected)))
        );
        assert_eq!(
            writer.poll_flush(&mut socket, &mut cx),
            Poll::Ready(Err(FramingError::Closed))
        );
        assert_eq!(
            writer.start(fixture(), FrameProtection::Tagged),
            Err(FramingError::Closed)
        );
    }
    let (mut socket, control) = ScriptedSocket::new();
    control.push_read(ReadStep::Error(ErrorKind::Interrupted));
    control.push_read(ReadStep::Error(ErrorKind::ConnectionReset));
    assert_eq!(
        read(
            &mut FrameReader::default(),
            &mut socket,
            FrameProtection::Untagged
        )
        .err(),
        Some(FramingError::Io(ErrorKind::ConnectionReset))
    );
}
