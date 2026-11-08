//! Paid wrong evidence, uncharged timeout and existing included entitlements through real accounting.
use super::{
    Error, accounting,
    authority::{Action, Authority},
    authorization::authorize,
    checks::{Agreement, Stamp},
    contract::{Limits, OperationKey},
    handler::{Blob, Check, Peer, Requester},
    persistence::Memory,
    quote::BoardCurrent,
    registry::{NoStandards, Registry},
    runtime::{Config, Coordinator, Payment, Phase},
    runtime_tests::context,
    test_support::check_family::Bytes,
    value::Value,
};
use crate::modules::{board, identity::api::IdentityId, ledger};
use std::{collections::BTreeMap, sync::Arc};
struct Fixture {
    runtime: Coordinator<Memory>,
    a: Arc<Authority>,
    registry: Arc<Registry>,
    ledger: Arc<ledger::state::Ledger<ledger::persistence::Memory>>,
    la: Arc<ledger::authority::Authority>,
    family: Arc<Bytes>,
    owner: board::state::Provider,
    selection: board::selection::Selection,
    ba: Arc<board::authority::Authority>,
}
// A pre-existing test-resource owner supplies check rules; this does not implement a Storage or Discovery family.
fn fixture(included: bool, wanted: bool) -> Fixture {
    let local = IdentityId([2; 32]);
    let a = Arc::new(Authority::new());
    let family = Arc::new(Bytes::new(included));
    let registry = Arc::new(
        Registry::new(local, a.clone(), Limits::default(), Arc::new(NoStandards)).unwrap(),
    );
    let rg = a.grant(local, Action::Register, 1_000_000).unwrap();
    registry
        .register(
            &context(&rg),
            family.clone(),
            if wanted { None } else { Some(family.clone()) },
        )
        .unwrap();
    let ba = Arc::new(board::authority::Authority::new());
    let owner =
        board::state::Provider::new(local, ba.clone(), Default::default(), registry.clone())
            .unwrap();
    let issuer = crate::modules::identity::text::format(local);
    let offering = serde_json::json!({"id":"echo","service":"org.example.echo","serviceVersion":1,"description":"Return the original text.","creditIssuer":issuer,"price":7,"checkPrice":3});
    let raw = if wanted {
        serde_json::json!({"version":1,"providedServices":[],"wantedServices":[offering]})
    } else {
        serde_json::json!({"version":1,"providedServices":[offering],"wantedServices":[]})
    };
    let pg = ba
        .grant(local, board::authority::Action::Publish, 1_000_000)
        .unwrap();
    let bc = board::contract::Context {
        grant: &pg,
        deadline: context(&rg).deadline,
        cancelled: false,
    };
    let prepared = board::prepared::Prepared::new(
        local,
        raw.to_string().as_bytes(),
        &BTreeMap::from([("echo".into(), family.definition().document.bytes().to_vec())]),
        Default::default(),
        registry.as_ref(),
    )
    .unwrap();
    let snapshot = owner.publish(&bc, prepared).unwrap();
    let sg = ba
        .grant(local, board::authority::Action::Select, 1_000_000)
        .unwrap();
    let selection = owner
        .select(
            &board::contract::Context { grant: &sg, ..bc },
            &snapshot,
            &board::selection::Request {
                id: "echo",
                service: "org.example.echo",
                version: 1,
                description: "Return the original text.",
                other: IdentityId([1; 32]),
                authenticated: true,
            },
        )
        .unwrap();
    let la = Arc::new(ledger::authority::Authority::new());
    let ag = la
        .grant(local, ledger::authority::Action::Administer, 1_000_000)
        .unwrap();
    let lc = ledger::contract::Context {
        version: 1,
        grant: &ag,
        deadline: bc.deadline,
        cancelled: false,
    };
    let ledger = Arc::new(
        ledger::state::Ledger::initialize(
            local,
            la.clone(),
            ledger::persistence::Memory::default(),
            Default::default(),
            &lc,
            1000,
            100,
        )
        .unwrap(),
    );
    ledger.grant(&lc, IdentityId([1; 32]), 100).unwrap();
    ledger
        .set_trust(
            &lc,
            IdentityId([1; 32]),
            ledger::accounting::Trust::new(1).unwrap(),
        )
        .unwrap();
    let cg = la
        .grant(local, ledger::authority::Action::Coordinate, 1_000_000)
        .unwrap();
    let adapter = Arc::new(accounting::Ledger::new(
        ledger.clone(),
        cg,
        Default::default(),
    ));
    let runtime = Coordinator::open(
        Config {
            local,
            authority: a.clone(),
            limits: Limits::default(),
        },
        &context(&rg),
        Memory::default(),
        adapter,
        registry.clone(),
        true,
    )
    .unwrap();
    Fixture {
        runtime,
        a,
        registry,
        ledger,
        la,
        family,
        owner,
        selection,
        ba,
    }
}
// Explicit local consent contains challenge only and exact quote bytes/hash; expected bytes are never part of it.
fn consent(f: &Fixture, stream: u32) -> (Agreement, Peer) {
    use sha2::{Digest, Sha256};
    let local = IdentityId([2; 32]);
    let requester = f.selection.roles().0;
    let requester_side = requester == local;
    let g =
        f.a.grant(
            local,
            if requester_side {
                Action::Request
            } else {
                Action::Invoke
            },
            1_000_000,
        )
        .unwrap();
    let peer = Peer {
        local,
        remote: IdentityId([1; 32]),
        key: OperationKey {
            requester,
            connection: [9; 32],
            stream,
        },
    };
    let challenge = Value::Uint64(0);
    let auth = authorize(
        &context(&g),
        &f.a,
        local,
        &f.registry,
        f.selection.clone(),
        peer,
        requester_side.then_some(challenge.clone()),
    )
    .unwrap();
    let bytes = Blob::new(br#"{"type":"check.quote"}"#.to_vec(), 128).unwrap();
    let hash = Sha256::digest(bytes.bytes()).into();
    (
        Agreement {
            authorized: auth,
            challenge,
            bytes,
            hash,
            maximum: 3,
            payment: Payment::default(),
        },
        peer,
    )
}
// Wrong but well-formed evidence is paid fail; missing expectations cannot become pass or refund the charge.
#[test]
fn paid_evidence_and_private_expectations() {
    let f = fixture(false, true);
    let local = IdentityId([2; 32]);
    let cg = f.a.grant(local, Action::Coordinate, 1_000_000).unwrap();
    let c = context(&cg);
    let observe = f.a.grant(local, Action::Observe, 1_000_000).unwrap();
    let bg =
        f.ba.grant(local, board::authority::Action::Select, 1_000_000)
            .unwrap();
    let bc = board::contract::Context {
        grant: &bg,
        deadline: c.deadline,
        cancelled: false,
    };
    let current = BoardCurrent {
        owner: &f.owner,
        context: &bc,
    };
    let (agreement, peer) = consent(&f, 1);
    let key = f.runtime.admit_check(&c, agreement, &current).unwrap();
    f.runtime.supply_input(&c, key, Value::Uint64(0)).unwrap();
    let (_, challenge) = f.runtime.outbound_check(&c, key).unwrap();
    assert_eq!(challenge, Value::Uint64(0));
    f.runtime
        .check_sent(
            &c,
            key,
            Stamp {
                epoch: [7; 32],
                milliseconds: 100,
            },
        )
        .unwrap();
    assert_eq!(
        f.runtime.receive_check(
            &c,
            peer,
            Value::Bytes(vec![]),
            Stamp {
                epoch: [7; 32],
                milliseconds: 200
            }
        ),
        Err(Error::Invalid)
    );
    f.runtime
        .receive_check(
            &c,
            peer,
            Value::Bytes(vec![b'x']),
            Stamp {
                epoch: [7; 32],
                milliseconds: 200,
            },
        )
        .unwrap();
    assert_eq!(f.runtime.resume(&c, key), Ok(Phase::Committed));
    assert_eq!(
        f.runtime
            .check_outcome(&context(&observe), key, Some(&Value::Bytes(vec![b'i']))),
        Ok(Check::Fail)
    );
    assert_eq!(
        f.runtime.check_outcome(&context(&observe), key, None),
        Ok(Check::Inconclusive)
    );
    let read =
        f.la.grant(local, ledger::authority::Action::Read, 1_000_000)
            .unwrap();
    let rc = ledger::contract::Context {
        version: 1,
        grant: &read,
        deadline: c.deadline,
        cancelled: false,
    };
    assert_eq!(f.ledger.view(&rc, peer.remote).unwrap().issued, 103);
    assert_eq!(
        f.ledger.view(&rc, peer.remote).unwrap().trust.millionths(),
        1
    );
    assert_eq!(f.family.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
}
// A real local provider sees only the challenge; an existing one-attempt entitlement never creates a zero-price Ledger operation.
#[test]
fn included_provider_evidence_and_exhaustion() {
    let f = fixture(true, false);
    let local = IdentityId([2; 32]);
    let cg = f.a.grant(local, Action::Coordinate, 1_000_000).unwrap();
    let c = context(&cg);
    let observe = f.a.grant(local, Action::Observe, 1_000_000).unwrap();
    let bg =
        f.ba.grant(local, board::authority::Action::Select, 1_000_000)
            .unwrap();
    let bc = board::contract::Context {
        grant: &bg,
        deadline: c.deadline,
        cancelled: false,
    };
    let current = BoardCurrent {
        owner: &f.owner,
        context: &bc,
    };
    let (agreement, _) = consent(&f, 1);
    let key = f.runtime.admit_check(&c, agreement, &current).unwrap();
    f.runtime.supply_input(&c, key, Value::Uint64(0)).unwrap();
    f.runtime.start(&c, key).unwrap();
    assert_eq!(f.runtime.resume(&c, key), Ok(Phase::Committed));
    assert_eq!(
        f.runtime.check_evidence(&context(&observe), key),
        Ok(Value::Bytes(vec![b'i']))
    );
    assert_eq!(*f.family.seen.lock().unwrap(), vec![Value::Uint64(0)]);
    let (again, _) = consent(&f, 2);
    assert_eq!(
        f.runtime.admit_check(&c, again, &current),
        Err(Error::Limit)
    );
    let lg =
        f.la.grant(local, ledger::authority::Action::Coordinate, 1_000_000)
            .unwrap();
    let lc = ledger::contract::Context {
        version: 1,
        grant: &lg,
        deadline: c.deadline,
        cancelled: false,
    };
    assert_eq!(
        f.ledger.status(
            &lc,
            ledger::contract::OperationKey {
                requester: key.requester,
                connection: key.connection,
                stream: key.stream
            }
        ),
        Ok(None)
    );
}
// No complete timely evidence is uncharged and inconclusive; late bytes cannot resurrect a completed timeout.
#[test]
fn timeout_without_valid_evidence() {
    let f = fixture(false, true);
    let local = IdentityId([2; 32]);
    let cg = f.a.grant(local, Action::Coordinate, 1_000_000).unwrap();
    let c = context(&cg);
    let observe = f.a.grant(local, Action::Observe, 1_000_000).unwrap();
    let bg =
        f.ba.grant(local, board::authority::Action::Select, 1_000_000)
            .unwrap();
    let bc = board::contract::Context {
        grant: &bg,
        deadline: c.deadline,
        cancelled: false,
    };
    let current = BoardCurrent {
        owner: &f.owner,
        context: &bc,
    };
    let (agreement, peer) = consent(&f, 1);
    let key = f.runtime.admit_check(&c, agreement, &current).unwrap();
    f.runtime.supply_input(&c, key, Value::Uint64(0)).unwrap();
    f.runtime.outbound_check(&c, key).unwrap();
    f.runtime
        .check_sent(
            &c,
            key,
            Stamp {
                epoch: [7; 32],
                milliseconds: 100,
            },
        )
        .unwrap();
    assert_eq!(
        f.runtime.check_timeout(
            &c,
            key,
            Stamp {
                epoch: [8; 32],
                milliseconds: 1101
            }
        ),
        Err(Error::Unknown)
    );
    f.runtime
        .check_timeout(
            &c,
            key,
            Stamp {
                epoch: [7; 32],
                milliseconds: 1101,
            },
        )
        .unwrap();
    assert_eq!(
        f.runtime.check_outcome(&context(&observe), key, None),
        Ok(Check::Inconclusive)
    );
    assert_eq!(
        f.runtime.receive_check(
            &c,
            peer,
            Value::Bytes(vec![b'i']),
            Stamp {
                epoch: [7; 32],
                milliseconds: 1102
            }
        ),
        Err(Error::Conflict)
    );
    let read =
        f.la.grant(local, ledger::authority::Action::Read, 1_000_000)
            .unwrap();
    let rc = ledger::contract::Context {
        version: 1,
        grant: &read,
        deadline: c.deadline,
        cancelled: false,
    };
    assert_eq!(f.ledger.view(&rc, peer.remote).unwrap().issued, 100);
    assert_eq!(f.ledger.budgets(&rc).unwrap().0, 900);
}

// A provider's durable charge and a requester-local uncharged timeout can coexist after response loss.
// This reproduces the language-independent clarification tracked by CR2SE bug0007, not an atomic payment guarantee.
#[test]
fn timeout_and_provider_commit_have_distinct_local_credit_views() {
    let server = fixture(false, false);
    let owner = IdentityId([1; 32]);
    let issuer = IdentityId([2; 32]);
    let authority = Arc::new(Authority::new());
    let family = Arc::new(Bytes::new(false));
    let registry = Arc::new(
        Registry::new(
            owner,
            authority.clone(),
            Limits::default(),
            Arc::new(NoStandards),
        )
        .unwrap(),
    );
    let register = authority.grant(owner, Action::Register, 1_000_000).unwrap();
    registry
        .register(&context(&register), family, None)
        .unwrap();
    let la = Arc::new(ledger::authority::Authority::new());
    let admin = la
        .grant(owner, ledger::authority::Action::Administer, 1_000_000)
        .unwrap();
    let lc = ledger::contract::Context {
        version: 1,
        grant: &admin,
        deadline: context(&register).deadline,
        cancelled: false,
    };
    let accounts = Arc::new(
        ledger::state::Ledger::initialize(
            owner,
            la.clone(),
            ledger::persistence::Memory::default(),
            Default::default(),
            &lc,
            1000,
            100,
        )
        .unwrap(),
    );
    accounts.set_owned(&lc, issuer, 100).unwrap();
    let bank = la
        .grant(owner, ledger::authority::Action::Coordinate, 1_000_000)
        .unwrap();
    let adapter = Arc::new(accounting::Ledger::new(
        accounts.clone(),
        bank,
        Default::default(),
    ));
    let client = Coordinator::open(
        Config {
            local: owner,
            authority: authority.clone(),
            limits: Limits::default(),
        },
        &context(&register),
        Memory::default(),
        adapter,
        registry.clone(),
        true,
    )
    .unwrap();
    let sg = server
        .ba
        .grant(issuer, board::authority::Action::Select, 1_000_000)
        .unwrap();
    let sc = board::contract::Context {
        grant: &sg,
        deadline: lc.deadline,
        cancelled: false,
    };
    let current = BoardCurrent {
        owner: &server.owner,
        context: &sc,
    };
    let (server_agreement, server_peer) = consent(&server, 1);
    let quote = server_agreement.bytes.clone();
    let hash = server_agreement.hash;
    let request = authority.grant(owner, Action::Request, 1_000_000).unwrap();
    let peer = Peer {
        local: owner,
        remote: issuer,
        key: server_peer.key,
    };
    let authorized = authorize(
        &context(&request),
        &authority,
        owner,
        &registry,
        server.selection.clone(),
        peer,
        Some(Value::Uint64(0)),
    )
    .unwrap();
    let cg = authority
        .grant(owner, Action::Coordinate, 1_000_000)
        .unwrap();
    let c = context(&cg);
    let key = client
        .admit_check(
            &c,
            Agreement {
                authorized,
                challenge: Value::Uint64(0),
                bytes: quote,
                hash,
                maximum: 3,
                payment: Payment::default(),
            },
            &current,
        )
        .unwrap();
    client.supply_input(&c, key, Value::Uint64(0)).unwrap();
    client.outbound_check(&c, key).unwrap();
    client
        .check_sent(
            &c,
            key,
            Stamp {
                epoch: [7; 32],
                milliseconds: 100,
            },
        )
        .unwrap();
    let grant = server
        .a
        .grant(issuer, Action::Coordinate, 1_000_000)
        .unwrap();
    let p = context(&grant);
    server
        .runtime
        .admit_check(&p, server_agreement, &current)
        .unwrap();
    server
        .runtime
        .supply_input(&p, key, Value::Uint64(0))
        .unwrap();
    server.runtime.start(&p, key).unwrap();
    assert_eq!(server.runtime.resume(&p, key), Ok(Phase::Committed));
    // Drop the evidence delivery entirely; the requester observes only its local deadline.
    client
        .check_timeout(
            &c,
            key,
            Stamp {
                epoch: [7; 32],
                milliseconds: 1101,
            },
        )
        .unwrap();
    let read = la
        .grant(owner, ledger::authority::Action::Read, 1_000_000)
        .unwrap();
    assert_eq!(
        accounts
            .view(&ledger::contract::Context { grant: &read, ..lc }, issuer)
            .unwrap()
            .owned,
        100
    );
    let read = server
        .la
        .grant(issuer, ledger::authority::Action::Read, 1_000_000)
        .unwrap();
    assert_eq!(
        server
            .ledger
            .view(
                &ledger::contract::Context {
                    version: 1,
                    grant: &read,
                    deadline: lc.deadline,
                    cancelled: false
                },
                owner
            )
            .unwrap()
            .issued,
        97
    );
}

// Concurrent distinct streams share one stable included entitlement; one winner never grants a second free check.
#[test]
fn concurrent_included_admission() {
    let f = fixture(true, false);
    let local = IdentityId([2; 32]);
    let g = f.a.grant(local, Action::Coordinate, 1_000_000).unwrap();
    let bg =
        f.ba.grant(local, board::authority::Action::Select, 1_000_000)
            .unwrap();
    let bc = board::contract::Context {
        grant: &bg,
        deadline: context(&g).deadline,
        cancelled: false,
    };
    let current = BoardCurrent {
        owner: &f.owner,
        context: &bc,
    };
    let (a, _) = consent(&f, 1);
    let (b, _) = consent(&f, 2);
    let gate = std::sync::Barrier::new(3);
    std::thread::scope(|scope| {
        let one = scope.spawn(|| {
            gate.wait();
            f.runtime.admit_check(&context(&g), a, &current)
        });
        let two = scope.spawn(|| {
            gate.wait();
            f.runtime.admit_check(&context(&g), b, &current)
        });
        gate.wait();
        let results = [one.join().unwrap(), two.join().unwrap()];
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert!(results.contains(&Err(Error::Limit)));
    });
    assert_eq!(f.family.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
}
