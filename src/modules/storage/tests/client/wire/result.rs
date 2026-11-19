//! Original complete peer output enters real requester coordination/accounting without claiming a running or durable remote Storage provider.
use super::*;
use wire::{record::Record, result_receiver::ResultReceiver};
// Begin one original provisional result receiver with scoped incoming authority; it exposes no successful application output.
pub(super) fn receiver(
    endpoint: &Endpoint,
    outgoing: &Outgoing,
) -> (wire::authority::Grant, ResultReceiver) {
    let invoke = endpoint
        .authority
        .grant(
            endpoint.association.local,
            wire::authority::Action::Invoke,
            16_777_216,
        )
        .unwrap();
    let receiver = ResultReceiver::new(
        &pc(&invoke),
        endpoint.authority.clone(),
        outgoing,
        WireLimits::default(),
    )
    .unwrap();
    (invoke, receiver)
}
// Simulate a complete remote value using the public wire codec, delivering all detached bytes before the explicit result.end marker.
pub(super) fn deliver(
    endpoint: &Endpoint,
    grant: &wire::authority::Grant,
    receiver: &mut ResultReceiver,
    output: &Value,
    price: u64,
    terminal: bool,
) {
    let encoded = wire::value::encode(output, WireLimits::default(), false).unwrap();
    let message = Message::parse(
        &serde_json::to_vec(
            &json!({"type":"result","output":encoded.metadata,"finalPrice":price.to_string()}),
        )
        .unwrap(),
        65536,
    )
    .unwrap();
    receiver
        .control(&pc(grant), endpoint.association, &message)
        .unwrap();
    for (id, body) in encoded.bodies.iter().enumerate() {
        receiver
            .body(
                &pc(grant),
                endpoint.association,
                &Record::Body {
                    id: id as u32,
                    offset: 0,
                    data: body.bytes().to_vec(),
                },
            )
            .unwrap();
    }
    if terminal {
        receiver
            .control(
                &pc(grant),
                endpoint.association,
                &Message::parse(br#"{"type":"result.end"}"#, 65536).unwrap(),
            )
            .unwrap();
    }
}
// Complete output is staged only after normal original END, then one explicit requester decision applies the exact local charge.
#[test]
fn complete_storage_outputs_reach_real_requester_coordination_once() {
    let (store, input, output) = store_fixture(false);
    let (renew, ri, ro, _) = renew_fixture();
    let (retrieve, gi, go, _) = retrieve();
    let (remove, di, do_, _) = remove_fixture();
    for (inv, input, output, price, factors) in [
        (store, input, output, 17, json!({})),
        (renew, ri, ro, 15, json!({"sizeBytes":"3"})),
        (retrieve, gi, go, 2, json!({})),
        (remove, di, do_, 2, json!({})),
    ] {
        let f = fixture();
        let endpoint = endpoint(f.client, &inv);
        let outgoing = endpoint.reserve(input.clone(), price, factors);
        assert_eq!(outgoing.input(), &input);
        assert_eq!(endpoint.available(), CREDIT - price);
        assert_eq!(endpoint.phase(), Some(services::runtime::Phase::Executing));
        let (grant, mut receiver) = receiver(&endpoint, &outgoing);
        deliver(&endpoint, &grant, &mut receiver, &output, price, true);
        assert_eq!(endpoint.phase(), Some(services::runtime::Phase::Executing));
        let coordinate = endpoint
            .services
            .grant(
                inv.requester,
                services::authority::Action::Coordinate,
                16_777_216,
            )
            .unwrap();
        receiver
            .finish(
                &pc(&grant),
                endpoint.association,
                true,
                &sc(&coordinate),
                &endpoint.runtime,
            )
            .unwrap();
        assert_eq!(endpoint.phase(), Some(services::runtime::Phase::Staged));
        endpoint.runtime.prepare(&sc(&coordinate), inv.key).unwrap();
        endpoint
            .runtime
            .decide(
                &sc(&coordinate),
                inv.key,
                services::runtime::Decision::Commit,
            )
            .unwrap();
        endpoint.runtime.commit(&sc(&coordinate), inv.key).unwrap();
        endpoint.runtime.commit(&sc(&coordinate), inv.key).unwrap();
        let observe = endpoint
            .services
            .grant(
                inv.requester,
                services::authority::Action::Observe,
                16_777_216,
            )
            .unwrap();
        let actual = endpoint
            .runtime
            .result(&sc(&observe), inv.key)
            .unwrap()
            .output;
        // Object member order is not protocol meaning; compare every complete typed field by name.
        let (Value::Object(actual), Value::Object(expected)) = (actual, output) else {
            panic!("Storage object result");
        };
        assert_eq!(
            actual.into_iter().collect::<BTreeMap<_, _>>(),
            expected.into_iter().collect::<BTreeMap<_, _>>()
        );
        assert_eq!(endpoint.available(), CREDIT - price);
    }
}
// Missing result.end, abnormal END or invalid original bytes retain the original unresolved local hold without publishing successful data.
#[test]
fn interrupted_or_incorrect_outputs_keep_original_holds() {
    for case in 0..3 {
        let f = fixture();
        let (inv, input, mut output, _) = retrieve();
        let endpoint = endpoint(f.client, &inv);
        let outgoing = endpoint.reserve(input, 2, json!({}));
        let (grant, mut receiver) = receiver(&endpoint, &outgoing);
        if case == 2 {
            set(&mut output, "data", Value::Bytes(b"abd".to_vec()));
        }
        deliver(&endpoint, &grant, &mut receiver, &output, 2, case != 0);
        let coordinate = endpoint
            .services
            .grant(
                inv.requester,
                services::authority::Action::Coordinate,
                16_777_216,
            )
            .unwrap();
        assert!(
            receiver
                .finish(
                    &pc(&grant),
                    endpoint.association,
                    case != 1,
                    &sc(&coordinate),
                    &endpoint.runtime
                )
                .is_err()
        );
        assert_eq!(endpoint.phase(), Some(services::runtime::Phase::Executing));
        assert_eq!(endpoint.available(), CREDIT - 2);
        let observe = endpoint
            .services
            .grant(
                inv.requester,
                services::authority::Action::Observe,
                16_777_216,
            )
            .unwrap();
        assert!(endpoint.runtime.result(&sc(&observe), inv.key).is_err());
    }
}

// Detached upload data cannot be queued until exact quote consent and reservation; framing remains bounded through arbitrary byte fragmentation.
#[test]
fn detached_store_sources_stay_with_original_reserved_attempt() {
    let f = fixture();
    let (mut inv, mut input, _) = store_fixture(false);
    let data = vec![7; 5000];
    set(&mut input, "sizeBytes", Value::Uint64(5000));
    set(&mut input, "data", Value::Bytes(data.clone()));
    set(
        &mut input,
        "contentHash",
        Value::Bytes(
            crate::modules::cryptography::primitives::hash::sha256(&data, data.len())
                .unwrap()
                .to_vec(),
        ),
    );
    inv.offering.source["info"]["maximumBytes"] = json!(6000);
    let endpoint = endpoint(f.client, &inv);
    let request = endpoint.request(input.clone(), CREDIT);
    let proposal = Message::parse(request.bytes(), 65536).unwrap();
    assert_eq!(proposal.value()["input"]["data"]["length"], "5000");
    assert_eq!(endpoint.available(), CREDIT);
    drop(request);
    let outgoing = endpoint.reserve(input, 27_500, json!({}));
    assert_eq!(endpoint.available(), CREDIT - 27_500);
    let mut sender =
        wire::input_transfer::InputSender::new(outgoing, WireLimits::default()).unwrap();
    let mut queue = wire::outbound::Outbound::new(512).unwrap();
    let mut decoder = wire::record::Decoder::default();
    let mut records = Vec::new();
    while sender.enqueue_next(&mut queue, 257).unwrap() {
        while queue.pending() > 0 {
            let bytes = queue.front(19).unwrap().to_vec();
            let mut offset = 0;
            while offset < bytes.len() {
                let (count, record) = decoder.push(&bytes[offset..]).unwrap();
                offset += count;
                if let Some(record) = record {
                    records.push(record);
                }
            }
            queue.advance(bytes.len()).unwrap();
        }
    }
    decoder.finish().unwrap();
    assert!(sender.queued());
    assert_eq!(queue.pending(), 0);
    let mut transferred = Vec::new();
    let mut controls = Vec::new();
    for record in records {
        match record {
            Record::Control(bytes) => controls.push(Message::parse(&bytes, 65536).unwrap().kind()),
            Record::Body { id, offset, data } => {
                assert_eq!(id, 0);
                assert_eq!(offset, transferred.len() as u64);
                transferred.extend(data);
            }
        }
    }
    assert_eq!(transferred, data);
    assert_eq!(
        controls,
        vec![wire::message::Kind::Accept, wire::message::Kind::InputEnd]
    );
    assert_eq!(endpoint.phase(), Some(services::runtime::Phase::Executing));
}

// A detached original retrieval stays provisional through the last byte and marker; only complete normal output reaches independent hash validation.
#[test]
fn detached_retrieval_requires_all_bytes_and_original_terminal_markers() {
    let f = fixture();
    let (mut inv, mut input, mut output, mut lease) = retrieve();
    let data = vec![9; 5000];
    let hash = crate::modules::cryptography::primitives::hash::sha256(&data, data.len()).unwrap();
    lease.size_bytes = 5000;
    lease.content_hash = hash;
    f.local
        .leases
        .lock()
        .unwrap()
        .insert((inv.key, inv.provider, inv.requester, lease.id), lease);
    inv.offering.source["info"]["maximumRetrieveBytes"] = json!(6000);
    set(&mut input, "length", Value::Uint64(5000));
    set(&mut output, "data", Value::Bytes(data.clone()));
    set(&mut output, "contentHash", Value::Bytes(hash.to_vec()));
    set(&mut output, "price", Value::Uint64(2500));
    let endpoint = endpoint(f.client, &inv);
    let outgoing = endpoint.reserve(input, 2500, json!({}));
    let (grant, mut receiver) = receiver(&endpoint, &outgoing);
    let encoded = wire::value::encode(&output, WireLimits::default(), false).unwrap();
    assert_eq!(encoded.bodies.len(), 1);
    let control = Message::parse(
        &serde_json::to_vec(
            &json!({"type":"result","output":encoded.metadata,"finalPrice":"2500"}),
        )
        .unwrap(),
        65536,
    )
    .unwrap();
    receiver
        .control(&pc(&grant), endpoint.association, &control)
        .unwrap();
    receiver
        .body(
            &pc(&grant),
            endpoint.association,
            &Record::Body {
                id: 0,
                offset: 0,
                data: data[..4999].to_vec(),
            },
        )
        .unwrap();
    let observe = endpoint
        .services
        .grant(
            inv.requester,
            services::authority::Action::Observe,
            16_777_216,
        )
        .unwrap();
    assert!(endpoint.runtime.result(&sc(&observe), inv.key).is_err());
    receiver
        .body(
            &pc(&grant),
            endpoint.association,
            &Record::Body {
                id: 0,
                offset: 4999,
                data: data[4999..].to_vec(),
            },
        )
        .unwrap();
    assert_eq!(endpoint.phase(), Some(services::runtime::Phase::Executing));
    receiver
        .control(
            &pc(&grant),
            endpoint.association,
            &Message::parse(br#"{"type":"result.end"}"#, 65536).unwrap(),
        )
        .unwrap();
    assert_eq!(endpoint.phase(), Some(services::runtime::Phase::Executing));
    let coordinate = endpoint
        .services
        .grant(
            inv.requester,
            services::authority::Action::Coordinate,
            16_777_216,
        )
        .unwrap();
    receiver
        .finish(
            &pc(&grant),
            endpoint.association,
            true,
            &sc(&coordinate),
            &endpoint.runtime,
        )
        .unwrap();
    assert_eq!(endpoint.phase(), Some(services::runtime::Phase::Staged));
    assert_eq!(endpoint.available(), CREDIT - 2500);
}
