//! Real module composition under test-owned authenticated scopes; network authentication is tested separately.
use super::*;
use crate::modules::transport::api::StreamOutcome;
use peer::{
    engine::bridge::test_support::{drain, ended, output},
    test_support::{
        invocation::Pair,
        world::{context, service_context},
    },
};
use services::value::Value;
use std::sync::atomic::Ordering;
// Bind only real independently owned module ports; custom Echo remains an explicitly installed test family.
fn endpoint<F>(
    pair: &Pair<F>,
) -> Endpoint<services::persistence::Memory, ledger::persistence::Memory> {
    endpoint_world(&pair.provider)
}
// Configure either independently owned endpoint without sharing its financial maps or authority issuers.
fn endpoint_world<F>(
    p: &peer::test_support::world::World<F>,
) -> Endpoint<services::persistence::Memory, ledger::persistence::Memory> {
    Endpoint::new(endpoint_config(p)).unwrap()
}
// Explicit configuration permits tests to install local wanted policy without altering other endpoint owners.
fn endpoint_config<F>(
    p: &peer::test_support::world::World<F>,
) -> Config<services::persistence::Memory, ledger::persistence::Memory> {
    Config {
        local: p.local,
        authority: p.authority.clone(),
        services: p.services.clone(),
        board: p.board.clone(),
        board_authority: p.board_authority.clone(),
        registry: p.registry.clone(),
        runtime: p.runtime.clone(),
        ledger: p.ledger.clone(),
        ledger_authority: p.ledger_authority.clone(),
        gate: p.gate.clone(),
        transfer_policy: Arc::new(peer::ledger_control::transfer::Fixed(0)),
        claims: None,
        wanted: None,
        resources: Default::default(),
        limits: Limits::default(),
        service_limits: Default::default(),
    }
}
// Recreate a trusted unit scope with the same original operation key, without deriving authority from wire data.
fn scope(a: Association) -> Scope {
    Scope {
        local: a.local,
        peer: a.peer,
        connection: a.operation.connection,
        stream: a.operation.stream,
        deadline: Instant::now() + Duration::from_secs(60),
    }
}
// Consume one sender record at a time without changing actual source ownership or input representation.
fn records(sender: &mut peer::input_transfer::InputSender) -> Vec<Record> {
    let mut queue = peer::outbound::Outbound::new(2_000_000).unwrap();
    while sender.enqueue_next(&mut queue, 65536).unwrap() {}
    let mut result = Vec::new();
    while let Some(bytes) = queue.front(usize::MAX) {
        let length = bytes.len();
        let mut decoder = peer::record::Decoder::default();
        let (count, record) = decoder.push(bytes).unwrap();
        assert_eq!(count, length);
        result.push(record.unwrap());
        queue.advance(count).unwrap();
    }
    result
}
// Drive actual endpoint handlers under explicit unit-only normal completion observations, never claim socket authentication here.
fn call(
    requester: &Endpoint<services::persistence::Memory, ledger::persistence::Memory>,
    provider: &Endpoint<services::persistence::Memory, ledger::persistence::Memory>,
    stream: u32,
    plan: requests::Plan,
) -> requests::Completion {
    let local = requester.0.config.local;
    let peer = provider.0.config.local;
    let client_scope = Scope {
        local,
        peer,
        connection: [19; 32],
        stream,
        deadline: Instant::now() + Duration::from_secs(60),
    };
    let server_scope = Scope {
        local: peer,
        peer: local,
        ..client_scope
    };
    let (call, completed) = requester.request(plan);
    let mut outgoing = output(2_000_000);
    let mut incoming = output(2_000_000);
    let mut client = call
        .open(client_scope, Direction::Local, &mut outgoing)
        .unwrap();
    let mut server = provider
        .open(server_scope, Direction::Remote, &mut incoming)
        .unwrap();
    let (mut client_end, mut server_end) = (false, false);
    for _ in 0..128 {
        for record in drain(&mut outgoing) {
            server.record(record, &mut incoming).unwrap();
        }
        if ended(&outgoing) && !client_end {
            server.remote_end(&mut incoming).unwrap();
            client_end = true;
        }
        for record in drain(&mut incoming) {
            client.record(record, &mut outgoing).unwrap();
        }
        if ended(&incoming) && !server_end {
            client.remote_end(&mut outgoing).unwrap();
            server_end = true;
        }
        assert!(matches!(
            completed.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
        if client_end && server_end {
            break;
        }
        if !ended(&outgoing) {
            client.writable(&mut outgoing).unwrap();
        }
        if !ended(&incoming) {
            server.writable(&mut incoming).unwrap();
        }
    }
    assert!(client_end && server_end);
    server.closed(StreamOutcome::Completed).unwrap();
    client.closed(StreamOutcome::Completed).unwrap();
    completed.try_recv().unwrap()
}

mod incoming;
mod invitations;
mod outgoing;
mod retries;

#[path = "resources.rs"]
mod resource_tests;

#[cfg(target_os = "linux")]
mod network;
