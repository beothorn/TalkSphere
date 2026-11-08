//! Bounded TRACE capture verifies Ledger inputs remain outside diagnostic output.
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
// Accepted terms, secret imports and failed parses remain absent from bounded developer TRACE output.
#[test]
fn ledger_trace_excludes_payloads_and_claims() {
    use crate::modules::ledger::{
        authority::{Action, Authority},
        claim::Portable,
        contract::{Context, Limits, OperationKey},
        persistence::Memory,
        state::Ledger,
        terms::{Policy, Terms},
    };
    use std::time::{Duration, Instant};
    let output = Output::default();
    let writer = output.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .without_time()
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        let [a, b, _, _] = super::test_support::identities();
        let authority = Arc::new(Authority::new());
        let admin = authority.grant(a, Action::Administer, 65536).unwrap();
        let coord = authority.grant(a, Action::Coordinate, 65536).unwrap();
        let context = |grant| Context {
            version: 1,
            grant,
            deadline: Instant::now() + Duration::from_secs(30),
            cancelled: false,
        };
        let ledger = Ledger::initialize(
            a,
            authority.clone(),
            Memory::default(),
            Limits {
                accounts: 8,
                operations: 8,
                transfers: 8,
                claims: 8,
                quotes: 8,
                state_bytes: 65536,
            },
            &context(&admin),
            100,
            100,
        )
        .unwrap();
        ledger.set_owned(&context(&admin), b, 100).unwrap();
        let r = ledger
            .reserve(
                &context(&coord),
                OperationKey {
                    requester: a,
                    connection: [1; 32],
                    stream: 1,
                },
                Terms {
                    requester: a,
                    provider: b,
                    issuer: b,
                    maximum: 7,
                    fixed: true,
                    accepted: b"SECRET_ACCEPTED_TERMS_MARKER".to_vec(),
                    transfer: None,
                    fee: None,
                    spending_bound: 7,
                },
                Policy::default(),
            )
            .unwrap();
        let p = ledger
            .prepare(
                &context(&coord),
                &r,
                7,
                b"SECRET_RESULT_REFERENCE_MARKER".to_vec(),
            )
            .unwrap();
        ledger.commit(&context(&coord), &p).unwrap();
        assert!(Portable::parse("SECRET_INVALID_BEARER_MARKER").is_err());
    });
    let bytes = output.0.lock().unwrap();
    assert!(!bytes.is_empty());
    assert!(bytes.len() < 131072);
    let text = String::from_utf8_lossy(&bytes);
    for marker in [
        "SECRET_ACCEPTED_TERMS_MARKER",
        "SECRET_RESULT_REFERENCE_MARKER",
        "SECRET_INVALID_BEARER_MARKER",
    ] {
        assert!(!text.contains(marker));
    }
}
