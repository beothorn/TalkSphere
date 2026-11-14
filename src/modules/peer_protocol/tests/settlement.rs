//! Independent issuer observations reconcile actual three-party Ledger books without forwarding receipts or repeating service work.
use super::{
    Error,
    authority::{Action, Authority},
    contract::{Association, Limits, OperationKey},
    ledger_control::{
        query::{Request, Response, Transfer},
        read,
        settlement::{self, Ports, Problem},
    },
    message::Message,
    record::Record,
    test_support::{
        ledger::Issuer,
        world::{context, ledger_context},
    },
};
use crate::modules::{
    identity::api::IdentityId,
    ledger::{
        self,
        accounting::Trust,
        payment::Payment,
        terms::{Policy, Terms},
    },
};
use std::sync::Arc;
struct Books {
    requester: ledger::state::Ledger<ledger::persistence::Memory>,
    provider: ledger::state::Ledger<ledger::persistence::Memory>,
    ra: Arc<ledger::authority::Authority>,
    pa: Arc<ledger::authority::Authority>,
    original: Association,
}
// Trusted coordinator fixtures prepare complete-result references using real finite requester/provider books and preagreed gross9/fee2/net7.
fn books(f: &Issuer) -> Books {
    let ra = Arc::new(ledger::authority::Authority::new());
    let pa = Arc::new(ledger::authority::Authority::new());
    let rg = ra
        .grant(f.owner, ledger::authority::Action::Administer, 16_777_216)
        .unwrap();
    let pg = pa
        .grant(
            f.recipient,
            ledger::authority::Action::Administer,
            16_777_216,
        )
        .unwrap();
    let requester = ledger::state::Ledger::initialize(
        f.owner,
        ra.clone(),
        ledger::persistence::Memory::default(),
        Default::default(),
        &ledger_context(&rg),
        1000,
        100,
    )
    .unwrap();
    let provider = ledger::state::Ledger::initialize(
        f.recipient,
        pa.clone(),
        ledger::persistence::Memory::default(),
        Default::default(),
        &ledger_context(&pg),
        1000,
        100,
    )
    .unwrap();
    requester
        .set_owned(&ledger_context(&rg), f.local, 100)
        .unwrap();
    provider
        .set_trust(&ledger_context(&pg), f.owner, Trust::new(1).unwrap())
        .unwrap();
    let original = Association {
        local: f.recipient,
        peer: f.owner,
        operation: OperationKey {
            requester: f.owner,
            connection: [8; 32],
            stream: 1,
        },
    };
    let key = ledger::contract::OperationKey {
        requester: f.owner,
        connection: [8; 32],
        stream: 1,
    };
    let terms = Terms {
        requester: f.owner,
        provider: f.recipient,
        issuer: f.local,
        maximum: 7,
        fixed: true,
        accepted: b"fixture complete accepted quote".to_vec(),
        transfer: Some(f.order.transfer_id),
        fee: Some(2),
        spending_bound: 9,
    };
    for (owner, auth, local) in [(&requester, &ra, f.owner), (&provider, &pa, f.recipient)] {
        let coordinate = auth
            .grant(local, ledger::authority::Action::Coordinate, 16_777_216)
            .unwrap();
        let c = ledger_context(&coordinate);
        let hold = owner
            .reserve(
                &c,
                key,
                terms.clone(),
                Policy {
                    third_party: true,
                    ..Default::default()
                },
            )
            .unwrap();
        let prepared = owner
            .prepare(
                &c,
                &hold,
                7,
                b"fixture retained complete-result reference".to_vec(),
            )
            .unwrap();
        owner.commit(&c, &prepared).unwrap();
    }
    Books {
        requester,
        provider,
        ra,
        pa,
        original,
    }
}
// Read a real protected issuer reply, then attribute it through the original requester query's authenticated completion.
fn observation(f: &Issuer, local: IdentityId, peer: IdentityId, changed_fee: bool) -> Transfer {
    let auth = Arc::new(Authority::new());
    let rg = auth.grant(local, Action::Request, 16_777_216).unwrap();
    let ig = auth.grant(local, Action::Invoke, 16_777_216).unwrap();
    let a = Association {
        local,
        peer,
        operation: OperationKey {
            requester: local,
            connection: [9; 32],
            stream: 3,
        },
    };
    let request =
        Request::transfer(&context(&rg), auth.clone(), a, f.owner, f.order.transfer_id).unwrap();
    let message = Message::parse(request.bytes(), 65536).unwrap();
    let pg = f.peer.grant(f.local, Action::Invoke, 16_777_216).unwrap();
    let lg = f
        .authority
        .grant(f.local, ledger::authority::Action::Control, 16_777_216)
        .unwrap();
    let server = Association {
        local: f.local,
        peer: local,
        operation: a.operation,
    };
    let Record::Control(bytes) = read::respond(
        &context(&pg),
        f.peer.as_ref(),
        f.ledger.as_ref(),
        &ledger_context(&lg),
        server,
        &message,
        Limits::default(),
    )
    .unwrap() else {
        panic!("expected transfer response")
    };
    let mut value = Message::parse(&bytes, 65536).unwrap().value().clone();
    if changed_fee {
        value["fee"] = serde_json::json!("3");
        value["receivedAmount"] = serde_json::json!("6");
    }
    let Response::Transfer(proof) = request
        .finish(
            &context(&ig),
            a,
            &Message::parse(value.to_string().as_bytes(), 65536).unwrap(),
            true,
        )
        .unwrap()
    else {
        panic!("expected issuer observation")
    };
    *proof
}
// Correct independent original-issuer evidence settles each local view once; repeated confirmation cannot recharge or mint credits.
#[test]
fn independent_issuer_settlement_applies_exact_payment_once() {
    let f = Issuer::new();
    let b = books(&f);
    f.commit();
    let authority = Arc::new(Authority::new());
    for (owner, auth, local, original) in [
        (
            &b.requester,
            &b.ra,
            f.owner,
            Association {
                local: f.owner,
                peer: f.recipient,
                ..b.original
            },
        ),
        (&b.provider, &b.pa, f.recipient, b.original),
    ] {
        let pg = authority
            .grant(local, Action::Coordinate, 16_777_216)
            .unwrap();
        let pc = context(&pg);
        let cg = auth
            .grant(local, ledger::authority::Action::Coordinate, 16_777_216)
            .unwrap();
        let cc = ledger_context(&cg);
        let eg = auth
            .grant(local, ledger::authority::Action::Evidence, 16_777_216)
            .unwrap();
        let ec = ledger_context(&eg);
        let action = settlement::next(&pc, authority.as_ref(), original, owner, &cc).unwrap();
        assert_eq!(action.order.amount, 9);
        assert_eq!(action.fee, 2);
        assert_eq!(action.submit, local == f.owner);
        if local == f.recipient {
            let rg = authority.grant(local, Action::Request, 16_777_216).unwrap();
            let a = Association {
                local,
                peer: f.local,
                operation: OperationKey {
                    requester: local,
                    connection: [9; 32],
                    stream: 3,
                },
            };
            assert!(
                settlement::query(&context(&rg), authority.clone(), original, a, owner, &cc)
                    .is_ok()
            );
        }
        let proof = observation(&f, local, f.local, false);
        // Concurrent independent confirmations converge on one original payment without double application.
        let barrier = std::sync::Barrier::new(2);
        std::thread::scope(|threads| {
            let handles: Vec<_> = (0..2)
                .map(|_| {
                    threads.spawn(|| {
                        barrier.wait();
                        settlement::confirm(
                            &pc,
                            authority.as_ref(),
                            original,
                            &proof,
                            Ports {
                                owner,
                                authority: auth.clone(),
                                evidence: &ec,
                                coordinate: &cc,
                            },
                        )
                    })
                })
                .collect();
            for handle in handles {
                handle.join().unwrap().unwrap();
            }
        });
        let key = ledger::contract::OperationKey {
            requester: f.owner,
            connection: [8; 32],
            stream: 1,
        };
        assert_eq!(owner.payment(&cc, key), Ok(Some(Payment::Settled)));
        let read = auth
            .grant(local, ledger::authority::Action::Read, 16_777_216)
            .unwrap();
        assert_eq!(
            owner.view(&ledger_context(&read), f.local).unwrap().owned,
            if local == f.owner { 91 } else { 7 }
        );
    }
}
// Requester-forwarded data and changed fees cannot settle work; refusal/unreachability retain the original committed operation and holds.
#[test]
fn forwarded_changed_and_unreachable_issuer_remain_unsettled() {
    let f = Issuer::new();
    let b = books(&f);
    f.commit();
    let local = f.recipient;
    let authority = Arc::new(Authority::new());
    let pg = authority
        .grant(local, Action::Coordinate, 16_777_216)
        .unwrap();
    let pc = context(&pg);
    let cg =
        b.pa.grant(local, ledger::authority::Action::Coordinate, 16_777_216)
            .unwrap();
    let cc = ledger_context(&cg);
    let eg =
        b.pa.grant(local, ledger::authority::Action::Evidence, 16_777_216)
            .unwrap();
    let ec = ledger_context(&eg);
    for (peer, changed) in [(f.owner, false), (f.local, true)] {
        let proof = observation(&f, local, peer, changed);
        assert_eq!(
            settlement::confirm(
                &pc,
                authority.as_ref(),
                b.original,
                &proof,
                Ports {
                    owner: &b.provider,
                    authority: b.pa.clone(),
                    evidence: &ec,
                    coordinate: &cc
                }
            ),
            Err(Error::Denied)
        );
    }
    let key = ledger::contract::OperationKey {
        requester: f.owner,
        connection: [8; 32],
        stream: 1,
    };
    for (problem, state) in [
        (Problem::Unreachable, Payment::Unknown),
        (Problem::Refused, Payment::Unsettled),
        (Problem::FeeChanged, Payment::Unsettled),
    ] {
        settlement::problem(
            &pc,
            authority.as_ref(),
            b.original,
            &b.provider,
            &cc,
            problem,
        )
        .unwrap();
        assert_eq!(b.provider.payment(&cc, key), Ok(Some(state)));
        assert_eq!(
            b.provider.status(&cc, key),
            Ok(Some(ledger::reservation::Status::Committed))
        );
    }
    let read =
        b.pa.grant(local, ledger::authority::Action::Read, 16_777_216)
            .unwrap();
    assert_eq!(
        b.provider
            .view(&ledger_context(&read), f.local)
            .unwrap()
            .owned,
        0
    );
    let request = authority.grant(local, Action::Request, 16_777_216).unwrap();
    let same = Association {
        local,
        peer: f.local,
        operation: OperationKey {
            requester: local,
            connection: b.original.operation.connection,
            stream: 3,
        },
    };
    assert!(matches!(
        settlement::query(
            &context(&request),
            authority.clone(),
            b.original,
            same,
            &b.provider,
            &cc
        ),
        Err(Error::Denied)
    ));
    let conflicting = Terms {
        requester: f.owner,
        provider: local,
        issuer: f.local,
        maximum: 7,
        fixed: true,
        accepted: b"second invocation".to_vec(),
        transfer: Some(f.order.transfer_id),
        fee: Some(2),
        spending_bound: 9,
    };
    assert!(matches!(
        b.provider.reserve(
            &cc,
            ledger::contract::OperationKey { stream: 5, ..key },
            conflicting,
            Policy {
                third_party: true,
                ..Default::default()
            }
        ),
        Err(ledger::Error::Conflict)
    ));
}
