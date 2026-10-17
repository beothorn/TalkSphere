//! Public header/payload composition and logging checks without a connection driver.

use std::sync::{Arc, Mutex};
use talksphere::modules::transport::codec::{
    FrameHeader, FrameProtection, FrameType,
    control::{ErrorCode, ErrorPayload, GoAway, PingToken},
};
use tracing::{
    Event, Metadata, Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
};

// A caller assembles exact headers and payloads for ping, cancellation and graceful draining.
#[test]
fn caller_can_compose_control_headers_and_payloads() {
    let cases = [
        (
            FrameType::Ping,
            0,
            PingToken::new([0, 1, 2, 3, 4, 5, 6, 7]).encode().to_vec(),
            vec![0, 1, 2, 3, 4, 5, 6, 7],
        ),
        (
            FrameType::Pong,
            0,
            PingToken::new([0, 1, 2, 3, 4, 5, 6, 7]).encode().to_vec(),
            vec![0, 1, 2, 3, 4, 5, 6, 7],
        ),
        (
            FrameType::Cancel,
            3,
            ErrorPayload::new(ErrorCode::Cancelled, "")
                .unwrap()
                .encode(),
            vec![0, 9, 0, 0],
        ),
        (
            FrameType::Error,
            0,
            ErrorPayload::new(ErrorCode::ProtocolError, "bad")
                .unwrap()
                .encode(),
            vec![0, 1, 0, 3, b'b', b'a', b'd'],
        ),
        (
            FrameType::GoAway,
            0,
            GoAway::new(3, ErrorCode::NoError, "").unwrap().encode(),
            vec![0, 0, 0, 3, 0, 0, 0, 0],
        ),
    ];
    for (kind, stream_id, payload, expected) in cases {
        assert_eq!(payload, expected);
        let header = FrameHeader::new(kind, stream_id, payload.len().try_into().unwrap()).unwrap();
        let wire = [header.encode().as_slice(), payload.as_slice()].concat();
        let received = FrameHeader::decode(&wire[..12]).unwrap();
        assert_eq!(
            received.frame_len(FrameProtection::Untagged),
            Ok(wire.len())
        );
        assert_eq!(
            received.frame_len(FrameProtection::Tagged),
            Ok(wire.len() + 16)
        );
        match kind {
            FrameType::Ping | FrameType::Pong => assert_eq!(
                PingToken::decode(&wire[12..]).unwrap().encode().as_slice(),
                expected
            ),
            FrameType::Error | FrameType::Cancel => assert_eq!(
                ErrorPayload::decode(&wire[12..]).unwrap().encode(),
                expected
            ),
            FrameType::GoAway => assert_eq!(
                GoAway::decode(&wire[12..])
                    .unwrap()
                    .last_accepted_remote_stream_id(),
                3
            ),
            _ => unreachable!("only control frames are in this scenario"),
        }
    }
}

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<String>>>);

impl Subscriber for Capture {
    // Record all codec events so a trace-level leak cannot hide behind a filter.
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }
    // These codecs emit events, but provide an ID if a span is added later.
    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }
    // No test scenario updates span fields.
    fn record(&self, _: &Id, _: &Record<'_>) {}
    // No test scenario uses causal span links.
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    // Capture target, level and every field without imposing an application logger.
    fn event(&self, event: &Event<'_>) {
        let mut fields = Fields(format!(
            "{} {}",
            event.metadata().target(),
            event.metadata().level()
        ));
        event.record(&mut fields);
        self.0.lock().unwrap().push(fields.0);
    }
    // Entry does not affect event content in this recording subscriber.
    fn enter(&self, _: &Id) {}
    // Exit does not affect event content in this recording subscriber.
    fn exit(&self, _: &Id) {}
}

struct Fields(String);
impl Visit for Fields {
    // Include field names as well as values when checking for leaked peer data.
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        use std::fmt::Write;
        write!(self.0, " {}={value:?}", field.name()).unwrap();
    }
}

// Operators can trace the codec flow without seeing reasons or opaque token contents.
#[test]
fn trace_events_have_context_without_raw_payloads_or_reasons() {
    let capture = Capture::default();
    tracing::subscriber::with_default(capture.clone(), || {
        let reason = "private diagnostic sentinel";
        let bytes = ErrorPayload::new(ErrorCode::Cancelled, reason)
            .unwrap()
            .encode();
        let decoded = ErrorPayload::decode(&bytes).unwrap();
        assert_eq!(decoded.reason(), reason);
        assert_eq!(decoded.code(), ErrorCode::Cancelled);
        let bytes = GoAway::new(3, ErrorCode::NoError, reason).unwrap().encode();
        let decoded = GoAway::decode(&bytes).unwrap();
        assert_eq!(decoded.reason(), reason);
        assert_eq!(decoded.code(), ErrorCode::NoError);
        assert_eq!(decoded.last_accepted_remote_stream_id(), 3);
        assert_eq!(
            PingToken::decode(&PingToken::new(*b"SECRETS!").encode())
                .unwrap()
                .encode(),
            *b"SECRETS!"
        );
    });
    let events = capture.0.lock().unwrap();
    assert!(!events.is_empty());
    for event in events.iter() {
        assert!(event.contains("talksphere::modules::transport::codec::control::"));
        assert!(event.contains("TRACE"));
        assert!(event.contains("function="));
        assert!(!event.contains("private diagnostic sentinel"));
        assert!(!event.contains("SECRETS!"));
    }
    for function in [
        "ErrorPayload::new",
        "ErrorPayload::decode",
        "GoAway::decode",
        "PingToken::decode",
    ] {
        assert!(events.iter().any(|event| event.contains(function)));
    }
}
