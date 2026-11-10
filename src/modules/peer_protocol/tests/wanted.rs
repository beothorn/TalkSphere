//! Wanted publisher policy supplies local input and spending while the prospective provider owns the stream opener role.
use super::{
    Error,
    acceptance::Admission,
    authority::Action,
    contract::{Association, Limits, OperationKey},
    exchange::{Direction, Exchange, Outcome},
    message::{Kind, Message},
    requester::Spending,
    result_receiver::ResultReceiver,
    test_support::{
        invocation::Pair,
        world::{context, service_context},
    },
    wanted::{self, Offerer, Policy, PublisherContexts, Work},
};
use crate::modules::{
    board,
    identity::{api::IdentityId, text},
    services::{self, handler::Requester, value::Value},
};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};
struct Application {
    calls: AtomicUsize,
    refuse: bool,
}
impl Policy for Application {
    // The only input comes from this locally installed application; peer-supplied fields never reach it as authority.
    fn choose(&self, _: &board::selection::Selection, _: IdentityId) -> Result<Work, Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.refuse {
            return Err(Error::Denied);
        }
        Ok(Work {
            input: Value::String("LOCAL_INPUT".into()),
            spending: Spending {
                maximum: 7,
                payment: Default::default(),
            },
        })
    }
}
// Publish and select an actual wanted Board with requester-owned credits from the prospective provider issuer.
fn selection(pair: &Pair) -> board::selection::Selection {
    let r = &pair.requester;
    let p = &pair.provider;
    let mut offering = r.family.offering(p.local).source;
    offering["creditIssuer"] = serde_json::json!(text::format(p.local));
    let board = serde_json::json!({"version":1,"providedServices":[],"wantedServices":[offering]})
        .to_string();
    let prepared = board::prepared::Prepared::new(
        r.local,
        board.as_bytes(),
        &BTreeMap::from([(
            "fixture".into(),
            r.family.definition().document.bytes().to_vec(),
        )]),
        Default::default(),
        r.registry.as_ref(),
    )
    .unwrap();
    let publish = r
        .board_authority
        .grant(r.local, board::authority::Action::Publish, 16_777_216)
        .unwrap();
    let read = r
        .board_authority
        .grant(r.local, board::authority::Action::Read, 16_777_216)
        .unwrap();
    let select = r
        .board_authority
        .grant(r.local, board::authority::Action::Select, 16_777_216)
        .unwrap();
    let c = board::contract::Context {
        grant: &publish,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    };
    r.board.publish(&c, prepared).unwrap();
    let snapshot = r
        .board
        .read(&board::contract::Context { grant: &read, ..c })
        .unwrap();
    let o = r.family.offering(p.local);
    r.board
        .select(
            &board::contract::Context {
                grant: &select,
                ..c
            },
            &snapshot,
            &board::selection::Request {
                id: "fixture",
                service: &o.service,
                version: 1,
                description: &o.description,
                other: p.local,
                authenticated: true,
            },
        )
        .unwrap()
}
// The opener provides work while the publisher requests it; real Services/Ledger commitments retain those reversed roles.
#[test]
fn wanted_invitation_uses_local_policy_and_reversed_roles() {
    let pair = Pair::new(false);
    let p = &pair.provider;
    let r = &pair.requester;
    let selected = selection(&pair);
    let pg = p
        .authority
        .grant(p.local, Action::Request, 16_777_216)
        .unwrap();
    let pc = context(&pg);
    let pi = p
        .authority
        .grant(p.local, Action::Invoke, 16_777_216)
        .unwrap();
    let pic = context(&pi);
    let psi = p
        .services
        .grant(p.local, services::authority::Action::Invoke, 16_777_216)
        .unwrap();
    let psic = service_context(&psi);
    let pco = p
        .services
        .grant(p.local, services::authority::Action::Coordinate, 16_777_216)
        .unwrap();
    let pcoc = service_context(&pco);
    let ri = r
        .authority
        .grant(r.local, Action::Invoke, 16_777_216)
        .unwrap();
    let ric = context(&ri);
    let rr = r
        .authority
        .grant(r.local, Action::Request, 16_777_216)
        .unwrap();
    let rrc = context(&rr);
    let rs = r
        .services
        .grant(r.local, services::authority::Action::Request, 16_777_216)
        .unwrap();
    let rsc = service_context(&rs);
    let rc = r
        .services
        .grant(r.local, services::authority::Action::Coordinate, 16_777_216)
        .unwrap();
    let rcc = service_context(&rc);
    let ro = r
        .services
        .grant(r.local, services::authority::Action::Observe, 16_777_216)
        .unwrap();
    let roc = service_context(&ro);
    let bg = r
        .board_authority
        .grant(r.local, board::authority::Action::Select, 16_777_216)
        .unwrap();
    let bc = board::contract::Context {
        grant: &bg,
        deadline: pc.deadline,
        cancelled: false,
    };
    let current = services::quote::BoardCurrent {
        owner: &r.board,
        context: &bc,
    };
    let pa = Association {
        local: p.local,
        peer: r.local,
        operation: OperationKey {
            requester: r.local,
            connection: [9; 32],
            stream: 2,
        },
    };
    let ra = Association {
        local: r.local,
        peer: p.local,
        ..pa
    };
    let offerer = Offerer::new(
        p.local,
        p.authority.clone(),
        p.services.clone(),
        p.registry.clone(),
        Limits::default(),
        p.gate.clone(),
    )
    .unwrap();
    let offered = offerer
        .offer(&pc, &psic, pa, selected.clone(), &current)
        .unwrap();
    assert_eq!(offered.association(), pa);
    let mut wire = Message::parse(offered.bytes(), 65536)
        .unwrap()
        .value()
        .clone();
    // Unknown optional installation/input suggestions cannot install code, choose local payloads or compel spending.
    wire["input"] = serde_json::json!("REMOTE_INPUT");
    wire["install"] = serde_json::json!("https://invalid.example/handler");
    let offer = Message::parse(wire.to_string().as_bytes(), 65536).unwrap();
    let app = Application {
        calls: AtomicUsize::new(0),
        refuse: false,
    };
    let request = wanted::request(
        &pair.client,
        PublisherContexts {
            incoming: &ric,
            request: &rrc,
            services: &rsc,
            current: &current,
        },
        ra,
        selected,
        &offer,
        &app,
    )
    .unwrap();
    let proposal_message = Message::parse(request.bytes(), 65536).unwrap();
    assert_eq!(proposal_message.value()["input"], "LOCAL_INPUT");
    assert_eq!(app.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        proposal_message.value()["publisherId"],
        text::format(r.local)
    );
    let proposal = offerer
        .proposal(&pic, &psic, offered, pa, &proposal_message)
        .unwrap();
    let issued = pair
        .issuer
        .issue(&pic, &psic, proposal, &current, pic.deadline)
        .unwrap();
    let quote = Message::parse(issued.bytes(), 65536).unwrap();
    let candidate = pair
        .client
        .validate(&ric, &rsc, request, ra, &quote)
        .unwrap();
    let hash = candidate.hash();
    let consent = pair.client.consent(&rrc, candidate, hash, 7).unwrap();
    let outgoing = pair
        .client
        .reserve(
            &rrc,
            Admission {
                quote_context: &rsc,
                coordinate_context: &rcc,
                current: &current,
                runtime: r.runtime.as_ref(),
            },
            consent,
        )
        .unwrap();
    assert_eq!(outgoing.invocation().requester, r.local);
    assert_eq!(outgoing.invocation().provider, p.local);
    assert_eq!(outgoing.invocation().publisher, r.local);
    let accepted = Message::parse(outgoing.accept(), 65536).unwrap();
    let incoming = pair
        .issuer
        .accept(
            &pic,
            Admission {
                quote_context: &psic,
                coordinate_context: &pcoc,
                current: &current,
                runtime: p.runtime.as_ref(),
            },
            issued,
            pa,
            &accepted,
        )
        .unwrap();
    pair.commit_provider(incoming, &Value::String("LOCAL_INPUT".into()));
    let mut result =
        ResultReceiver::new(&ric, r.authority.clone(), &outgoing, Limits::default()).unwrap();
    result
        .control(
            &ric,
            ra,
            &Message::parse(
                br#"{"type":"result","output":"LOCAL_INPUT","finalPrice":"7"}"#,
                65536,
            )
            .unwrap(),
        )
        .unwrap();
    result
        .control(
            &ric,
            ra,
            &Message::parse(br#"{"type":"result.end"}"#, 65536).unwrap(),
        )
        .unwrap();
    result
        .finish(&ric, ra, true, &rcc, r.runtime.as_ref())
        .unwrap();
    let key = super::acceptance::peer(ra).key;
    r.runtime.prepare(&rcc, key).unwrap();
    r.runtime
        .decide(&rcc, key, services::runtime::Decision::Commit)
        .unwrap();
    r.runtime.commit(&rcc, key).unwrap();
    assert_eq!(
        r.runtime.result(&roc, key).unwrap().output,
        Value::String("LOCAL_INPUT".into())
    );
    assert_eq!(pair.owned_available(), 93);
    assert_eq!(p.available(), 93);
    let mut ordering = Exchange::new(Direction::Local);
    for (direction, kind) in [
        (Direction::Local, Kind::Offer),
        (Direction::Remote, Kind::Propose),
        (Direction::Local, Kind::Quote),
        (Direction::Remote, Kind::Accept),
        (Direction::Remote, Kind::InputEnd),
    ] {
        ordering.control(direction, kind).unwrap();
    }
    ordering.end(Direction::Remote).unwrap();
    ordering.control(Direction::Local, Kind::Result).unwrap();
    ordering.control(Direction::Local, Kind::ResultEnd).unwrap();
    ordering.end(Direction::Local).unwrap();
    assert_eq!(ordering.close(true), Ok(Outcome::Complete));
}
// Malformed invitations and refused local policy produce no proposal, hold or execution.
#[test]
fn wanted_refusal_does_not_authorize_input_or_spending() {
    let pair = Pair::new(false);
    let r = &pair.requester;
    let p = &pair.provider;
    let selected = selection(&pair);
    let ri = r
        .authority
        .grant(r.local, Action::Invoke, 16_777_216)
        .unwrap();
    let ric = context(&ri);
    let rr = r
        .authority
        .grant(r.local, Action::Request, 16_777_216)
        .unwrap();
    let rrc = context(&rr);
    let rs = r
        .services
        .grant(r.local, services::authority::Action::Request, 16_777_216)
        .unwrap();
    let rsc = service_context(&rs);
    let bg = r
        .board_authority
        .grant(r.local, board::authority::Action::Select, 16_777_216)
        .unwrap();
    let bc = board::contract::Context {
        grant: &bg,
        deadline: rrc.deadline,
        cancelled: false,
    };
    let current = services::quote::BoardCurrent {
        owner: &r.board,
        context: &bc,
    };
    let a = Association {
        local: r.local,
        peer: p.local,
        operation: OperationKey {
            requester: r.local,
            connection: [1; 32],
            stream: 2,
        },
    };
    for case in 0..5 {
        let mut wire = serde_json::json!({"type":"offer","offeringId":"fixture","service":selected.offering().service,"serviceVersion":1});
        match case {
            0 => wire["offeringId"] = serde_json::json!("other"),
            1 => wire["service"] = serde_json::json!("other"),
            2 => wire["serviceVersion"] = serde_json::json!(2),
            _ => {}
        }
        let app = Application {
            calls: AtomicUsize::new(0),
            refuse: true,
        };
        let incoming = if case == 3 { &rrc } else { &ric };
        let message = Message::parse(wire.to_string().as_bytes(), 65536).unwrap();
        assert!(
            wanted::request(
                &pair.client,
                PublisherContexts {
                    incoming,
                    request: &rrc,
                    services: &rsc,
                    current: &current
                },
                a,
                selected.clone(),
                &message,
                &app
            )
            .is_err()
        );
        assert_eq!(
            app.calls.load(Ordering::SeqCst),
            if case == 4 { 1 } else { 0 }
        );
    }
    // Understanding a requester schema is insufficient to offer work without an actual installed provider.
    let registry = std::sync::Arc::new(
        services::registry::Registry::new(
            p.local,
            p.services.clone(),
            Default::default(),
            std::sync::Arc::new(services::registry::NoStandards),
        )
        .unwrap(),
    );
    let register = p
        .services
        .grant(p.local, services::authority::Action::Register, 16_777_216)
        .unwrap();
    registry
        .register(&service_context(&register), p.family.clone(), None)
        .unwrap();
    let offerer = Offerer::new(
        p.local,
        p.authority.clone(),
        p.services.clone(),
        registry,
        Limits::default(),
        p.gate.clone(),
    )
    .unwrap();
    let pg = p
        .authority
        .grant(p.local, Action::Request, 16_777_216)
        .unwrap();
    let sg = p
        .services
        .grant(p.local, services::authority::Action::Invoke, 16_777_216)
        .unwrap();
    let pa = Association {
        local: p.local,
        peer: r.local,
        ..a
    };
    assert!(matches!(
        offerer.offer(&context(&pg), &service_context(&sg), pa, selected, &current),
        Err(Error::Unsupported)
    ));
    assert_eq!(pair.owned_available(), 100);
    assert_eq!(p.available(), 100);
    assert_eq!(p.family.executions.load(Ordering::SeqCst), 0);
}
