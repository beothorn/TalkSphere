//! Aggregate admission and trace privacy are verified across endpoint clones, peers and callback failures.
use super::*;
use std::sync::{Barrier, Mutex};
// Aggregate capacity is atomic across threads and per-peer caps cannot be evaded by reconnecting.
#[test]
fn concurrent_endpoint_admission_reserves_capacity_until_handler_drop() {
    let limits = Limits {
        exchanges: 8,
        state_bytes: 8194,
        bodies: 2,
        ..Default::default()
    };
    let pool = resources::Pool::new(
        limits,
        resources::Budgets {
            value_bytes: 4 * limits.state_bytes * 4,
            bodies: 16,
            per_peer: 1,
        },
    )
    .unwrap();
    let arrived = Barrier::new(9);
    std::thread::scope(|threads| {
        let mut joins = Vec::new();
        for index in 0..8 {
            let pool = &pool;
            let arrived = &arrived;
            joins.push(threads.spawn(move || {
                let admitted = pool.open(IdentityId([if index < 4 { 1 } else { 2 }; 32]), || {
                    Ok(Box::new(Noop))
                });
                arrived.wait();
                let result = admitted.is_ok();
                drop(admitted);
                result
            }));
        }
        arrived.wait();
        assert_eq!(
            joins
                .into_iter()
                .map(|join| join.join().unwrap())
                .filter(|admitted| *admitted)
                .count(),
            2
        );
    });
    for _ in 0..8 {
        assert!(
            pool.open(IdentityId([1; 32]), || Ok(Box::new(Noop)))
                .is_ok()
        );
    }
    // Refused constructors return capacity without inventing a handler or financial rollback.
    assert!(matches!(
        pool.open(IdentityId([1; 32]), || Err(Error::Denied)),
        Err(Error::Denied)
    ));
    assert!(
        pool.open(IdentityId([1; 32]), || Ok(Box::new(Noop)))
            .is_ok()
    );
}
struct Noop;
impl Handler for Noop {
    // This inert callback checks only admission ownership, never authentication or service conformance.
    fn record(&mut self, _: Record, _: &mut Output) -> Result<(), Error> {
        Ok(())
    }
    // END has no additional effect in the admission-only helper.
    fn remote_end(&mut self, _: &mut Output) -> Result<(), Error> {
        Ok(())
    }
    // No output is produced by this admission-only helper.
    fn writable(&mut self, _: &mut Output) -> Result<(), Error> {
        Ok(())
    }
    // Cleanup records no financial state and permits release only when the wrapper drops.
    fn closed(&mut self, _: StreamOutcome) -> Result<(), Error> {
        Ok(())
    }
}
// Zero, overflowing and sub-exchange budgets fail startup instead of admitting unbounded bodies.
#[test]
fn aggregate_configuration_is_finite_and_shared_by_endpoint_clones() {
    for budgets in [
        resources::Budgets {
            value_bytes: 0,
            ..Default::default()
        },
        resources::Budgets {
            bodies: 0,
            ..Default::default()
        },
        resources::Budgets {
            per_peer: 0,
            ..Default::default()
        },
        resources::Budgets {
            value_bytes: 1,
            ..Default::default()
        },
        resources::Budgets {
            bodies: 1,
            ..Default::default()
        },
    ] {
        assert!(resources::Pool::new(Default::default(), budgets).is_err());
    }
    assert!(
        resources::Pool::new(
            Limits {
                state_bytes: usize::MAX / 2,
                ..Default::default()
            },
            Default::default()
        )
        .is_err()
    );
    let pair = Pair::new(false);
    let mut config = endpoint_config(&pair.provider);
    config.resources.per_peer = 1;
    let endpoint = Endpoint::new(config).unwrap();
    let cloned = endpoint.clone();
    let a = scope(pair.provider.proposal(1, &Value::String("x".into())).0);
    let mut output = output(2_000_000);
    let mut first = endpoint.open(a, Direction::Remote, &mut output).unwrap();
    first.closed(StreamOutcome::Completed).unwrap();
    assert!(matches!(
        cloned.open(
            Scope {
                stream: 3,
                connection: [91; 32],
                ..a
            },
            Direction::Remote,
            &mut output
        ),
        Err(Error::Limit)
    ));
    drop(first);
    assert!(
        cloned
            .open(Scope { stream: 5, ..a }, Direction::Remote, &mut output)
            .is_ok()
    );
}
#[derive(Clone)]
struct Capture(Arc<Mutex<Vec<u8>>>);
impl std::io::Write for Capture {
    // Keep trace evidence bounded even if a regression emits too much output.
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let mut captured = self.0.lock().unwrap();
        if captured.len() + bytes.len() > 1_000_000 {
            return Err(std::io::Error::other("trace budget"));
        }
        captured.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    // Captured trace has no external flushing side effects.
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
// Full TRACE instrumentation must describe operations without disclosing raw application values or ciphertext controls.
#[test]
fn trace_diagnostics_omit_payloads_and_claim_envelopes() {
    let capture = Capture(Arc::new(Mutex::new(Vec::new())));
    let writer = capture.clone();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_max_level(tracing::Level::TRACE)
        .with_writer(move || writer.clone())
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        let pair = Pair::new(false);
        let provider = endpoint(&pair);
        let requester = endpoint_world(&pair.requester);
        let secret = "PRIVATE_PAYLOAD_SENTINEL_do_not_record";
        let (issued, request) = pair.propose(1, Value::String(secret.into()), 7);
        let association = issued.association();
        let outgoing = pair.reserve(&issued, request).unwrap();
        let incoming = pair
            .accept(
                issued,
                association,
                &Message::parse(outgoing.accept(), 65536).unwrap(),
            )
            .unwrap();
        pair.commit_provider(incoming, &Value::String(secret.into()));
        let envelope = vec![53; ledger::redemption::ENVELOPE_BYTES];
        let _ = call(
            &requester,
            &provider,
            3,
            requests::Plan::Claim {
                envelope: envelope.clone(),
                id: [1; 32],
            },
        );
        let text = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
        assert!(!text.contains(secret));
        assert!(!text.contains(&peer::base64::encode(&envelope)));
        assert!(text.contains("claim"));
        assert!(text.len() < 1_000_000);
    });
}

// Many legal one-byte detached bodies must reserve references independently of the inline encoding threshold.
#[test]
fn small_detached_bodies_are_charged_against_aggregate_reference_capacity() {
    let limits = Limits {
        state_bytes: 16,
        nodes: 9,
        bodies: 8,
        exchanges: 4,
        ..Default::default()
    };
    let metadata = serde_json::Value::Array(
        (0..8)
            .map(|id| serde_json::json!({"$bytes":id,"length":"1"}))
            .collect(),
    );
    let pending = peer::value::Pending::parse(
        &metadata,
        &serde_json::json!({"type":"array","items":{"type":"bytes"}}),
        limits,
        false,
    )
    .unwrap();
    assert_eq!(pending.descriptors().len(), 8);
    let pool = resources::Pool::new(
        limits,
        resources::Budgets {
            value_bytes: 256,
            bodies: 16,
            per_peer: 4,
        },
    )
    .unwrap();
    let first = pool
        .open(IdentityId([1; 32]), || Ok(Box::new(Noop)))
        .unwrap();
    assert!(matches!(
        pool.open(IdentityId([2; 32]), || Ok(Box::new(Noop))),
        Err(Error::Limit)
    ));
    drop(first);
    assert!(
        pool.open(IdentityId([2; 32]), || Ok(Box::new(Noop)))
            .is_ok()
    );
}
