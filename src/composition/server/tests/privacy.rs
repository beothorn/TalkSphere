//! Native protected traffic does not expose private fixture keys, payloads or peer diagnostic text at TRACE.
use super::*;
use std::sync::{Arc, Mutex};
use tracing::{
    Event as TraceEvent, Metadata, Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
};
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
    fn event(&self, event: &TraceEvent<'_>) {
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

// Capture complete native connection workflows, including authenticated data and a malicious diagnostic reason.
#[test]
fn native_runtime_traces_exclude_payloads_keys_and_peer_diagnostics() {
    let capture = Capture::default();
    tracing::subscriber::with_default(capture.clone(), || {
        let mut h = Harness::new();
        h.receive(
            FrameType::Open,
            2,
            Open::new("cr2se.peer", 1).unwrap().encode(),
            None,
        );
        h.receive(
            FrameType::Data,
            2,
            b"DO_NOT_LOG_APPLICATION_PAYLOAD".to_vec(),
            None,
        );
        h.receive(
            FrameType::Error,
            0,
            ErrorPayload::new(ErrorCode::ProtocolError, "DO_NOT_TRUST_PEER_DIAGNOSTIC")
                .unwrap()
                .encode(),
            None,
        );
        assert!(h.control.is_closed());
    });
    let logs = capture.0.lock().unwrap().join("\n");
    assert!(logs.contains("TRACE"));
    for forbidden in [
        "DO_NOT_LOG_APPLICATION_PAYLOAD".to_string(),
        "DO_NOT_TRUST_PEER_DIAGNOSTIC".into(),
        "TSID0001".into(),
        crate::modules::cryptography::test_support::transport("initiator Ed25519 seed")
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        format!("{:?}", vector("initiator raw X25519 private-key input")),
    ] {
        assert!(!logs.contains(&forbidden));
    }
}
