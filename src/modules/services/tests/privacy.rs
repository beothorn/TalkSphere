//! Bounded TRACE capture verifies Services inputs remain outside diagnostic output.
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
// Arbitrary payload and private expectations remain absent from bounded Services TRACE output.
#[test]
fn services_trace_excludes_payloads() {
    use super::{
        codec, validation,
        value::{Budget, Value},
    };
    let output = Output::default();
    let writer = output.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .without_time()
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        let value = Value::String("SECRET_BODY_MARKER\nFORGED_LOG_MARKER".into());
        let budget = Budget {
            depth: 31,
            nodes: 128,
            bytes: 4096,
        };
        let bytes = codec::encode(&value, budget, 4096).unwrap();
        assert_eq!(codec::decode(&bytes, budget, 4096), Ok(value.clone()));
        assert!(
            validation::validate(&value, &serde_json::json!({"type":"string"}), budget, false)
                .is_ok()
        );
    });
    let bytes = output.0.lock().unwrap();
    assert!(!bytes.is_empty());
    assert!(bytes.len() < 131072);
    let text = String::from_utf8_lossy(&bytes);
    assert!(!text.contains("SECRET_BODY_MARKER"));
    assert!(!text.contains("FORGED_LOG_MARKER"));
}
