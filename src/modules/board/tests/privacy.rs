//! TRACE capture verifies Board metadata stays out of operational logs.
use std::sync::{Arc, Mutex};
#[derive(Clone, Default)]
struct Output(Arc<Mutex<Vec<u8>>>);
impl std::io::Write for Output {
    // Bound capture storage independently of the logger's input to expose accidental amplification.
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let mut output = self.0.lock().unwrap();
        if output.len() + bytes.len() > 131072 {
            return Err(std::io::Error::other("trace capture bound"));
        }
        output.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    // Writes are immediately visible in the test-owned shared buffer.
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
// Untrusted hints and unknown nested fields are retained as data, never emitted into TRACE logs.
#[test]
fn board_trace_excludes_raw_metadata() {
    let output = Output::default();
    let writer = output.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .without_time()
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        let bytes=br#"{"version":1,"providedServices":[],"wantedServices":[],"unknown":{"private":"TOP_SECRET_METADATA","instructions":"INSTALL_AND_SPEND_MARKER"}}"#;
        let b = super::offering::Board::parse(bytes, super::config::Limits::default()).unwrap();
        assert_eq!(b.document.bytes(), bytes);
        assert!(
            super::offering::Board::parse(
                b"INVALID_SECRET_MARKER",
                super::config::Limits::default()
            )
            .is_err()
        );
    });
    let bytes = output.0.lock().unwrap();
    assert!(!bytes.is_empty());
    assert!(bytes.len() < 131072);
    let text = String::from_utf8_lossy(&bytes);
    for marker in [
        "TOP_SECRET_METADATA",
        "INSTALL_AND_SPEND_MARKER",
        "INVALID_SECRET_MARKER",
    ] {
        assert!(!text.contains(marker));
    }
}
