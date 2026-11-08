//! Versioned invocation-record recovery rejects contradictions independently of live state transitions.
use super::{
    Error, codec,
    contract::{Limits, OperationKey},
    handler::{Blob, Completion},
    operation::{self, Operation, Phase},
    pricing::Price,
    value::{Budget, Value},
};
use crate::modules::{identity::api::IdentityId, ledger::terms::Terms};
use std::collections::BTreeMap;
// Construct a literal accepted binding without deriving expected states from the decoder.
fn record() -> Operation {
    let a = IdentityId([1; 32]);
    let b = IdentityId([2; 32]);
    let key = OperationKey {
        requester: a,
        connection: [8; 32],
        stream: 1,
    };
    Operation {
        key,
        service: "org.example.echo".into(),
        version: 1,
        terms: Terms {
            requester: a,
            provider: b,
            issuer: b,
            maximum: 7,
            fixed: true,
            accepted: vec![1],
            transfer: None,
            fee: None,
            spending_bound: 7,
        },
        definition: vec![1],
        metadata: Value::String("input".into()),
        pricing: Price::Fixed(7),
        input_bytes: 128,
        factors: Value::Object(vec![]),
        phase: Phase::Accepted,
        input: None,
        completion: None,
        handler: None,
    }
}
// Recovered phases cannot invent missing input, validated completion or preparation tokens.
#[test]
fn phase_prerequisites() {
    let l = Limits::default();
    let op = record();
    let records = BTreeMap::from([(op.key, op.clone())]);
    let bytes = operation::encode(&records, l).unwrap();
    assert_eq!(
        operation::decode(&bytes, l).unwrap()[&op.key].phase,
        Phase::Accepted
    );
    for phase in [
        Phase::Executing,
        Phase::Staged,
        Phase::Prepared,
        Phase::Commit,
        Phase::Committed,
    ] {
        let mut bad = op.clone();
        bad.phase = phase;
        assert!(
            operation::decode(
                &operation::encode(&BTreeMap::from([(bad.key, bad)]), l).unwrap(),
                l
            )
            .is_err()
        );
    }
    let mut bad = op.clone();
    bad.phase = Phase::Staged;
    bad.input = Some(Value::String("input".into()));
    bad.completion = Some(Completion {
        output: Value::String("input".into()),
        result: Blob::new(vec![1], 1).unwrap(),
        price: 8,
        effect: Blob::new(vec![1], 1).unwrap(),
    });
    assert!(
        operation::decode(
            &operation::encode(&BTreeMap::from([(bad.key, bad)]), l).unwrap(),
            l
        )
        .is_err()
    );
    let budget = Budget {
        depth: 31,
        nodes: l.nodes,
        bytes: l.state_bytes,
    };
    let mut value = codec::decode(&bytes, budget, l.state_bytes).unwrap();
    if let Value::Array(root) = &mut value {
        root[0] = Value::Uint32(2);
    }
    let future = codec::encode(&value, budget, l.state_bytes).unwrap();
    assert!(matches!(
        operation::decode(&future, l),
        Err(Error::Unsupported)
    ));
    let mut value = codec::decode(&bytes, budget, l.state_bytes).unwrap();
    if let Value::Array(root) = &mut value
        && let Value::Array(entries) = &mut root[1]
    {
        entries.push(entries[0].clone());
    }
    assert!(matches!(
        operation::decode(&codec::encode(&value, budget, l.state_bytes).unwrap(), l),
        Err(Error::Invalid)
    ));
}
