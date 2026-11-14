//! Real signature/issuer accounting follows exact same-stream fee acceptance; stored retries bypass new fee policy.
use super::{
    Error,
    authority::{Action, Authority},
    contract::{Association, Limits, OperationKey},
    exchange::{Direction, Exchange, Outcome},
    ledger_control::{
        transfer::{self, Begin, Time},
        transfer_client::{Request, Signed},
    },
    message::{Kind, Message},
    record::Record,
    test_support::{
        ledger::Issuer,
        world::{context, ledger_context},
    },
};
use crate::modules::ledger;
use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};
struct Policy {
    fee: AtomicU64,
    calls: AtomicUsize,
}
impl transfer::Policy for Policy {
    // Trusted issuer configuration is observed once for new work and never for accepted signed retries.
    fn fee(&self, _: &ledger::transfer::Order) -> Result<u64, Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.fee.load(Ordering::SeqCst))
    }
}
// Owner-signed fields are literal and canonical; no constructor bypasses Ledger cryptographic validation.
fn signed(f: &Issuer, maximum_fee: u64) -> Signed {
    Signed {
        order: f.order.clone(),
        key: f.key,
        signature: f.signature,
        maximum_fee,
    }
}
// Independent local roles share only the trusted original connection/stream observation.
fn route(f: &Issuer, stream: u32) -> (Association, Association) {
    let operation = OperationKey {
        requester: f.owner,
        connection: [8; 32],
        stream,
    };
    (
        Association {
            local: f.owner,
            peer: f.local,
            operation,
        },
        Association {
            local: f.local,
            peer: f.owner,
            operation,
        },
    )
}
// Extract only a complete expected control record; these exchanges never contain service bodies.
fn message(record: &Record) -> Message {
    let Record::Control(bytes) = record else {
        panic!("expected control")
    };
    Message::parse(bytes, 65536).unwrap()
}
// Configure a real issuer with the same independent authority as its Ledger and an explicit finite fee policy.
fn server(
    f: &Issuer,
    policy: Arc<dyn transfer::Policy>,
) -> transfer::Issuer<ledger::persistence::Memory> {
    transfer::Issuer::new(transfer::Config {
        peer: f.peer.clone(),
        authority: f.authority.clone(),
        ledger: f.ledger.clone(),
        limits: Limits::default(),
        policy,
    })
    .unwrap()
}
// New transfer commits only after explicit exact fee consent; accepted retry returns original gross/fee/net without consulting changed policy.
#[test]
fn exact_fee_consent_and_original_stored_retry() {
    let f = Issuer::new();
    let policy = Arc::new(Policy {
        fee: AtomicU64::new(2),
        calls: AtomicUsize::new(0),
    });
    let server = server(&f, policy.clone());
    let auth = Arc::new(Authority::new());
    let request = auth.grant(f.owner, Action::Request, 16_777_216).unwrap();
    let rc = context(&request);
    let incoming = auth.grant(f.owner, Action::Invoke, 16_777_216).unwrap();
    let ic = context(&incoming);
    let pg = f.peer.grant(f.local, Action::Invoke, 16_777_216).unwrap();
    let pc = context(&pg);
    let lg = f
        .authority
        .grant(f.local, ledger::authority::Action::Control, 16_777_216)
        .unwrap();
    let lc = ledger_context(&lg);
    let (ca, sa) = route(&f, 3);
    let mut client = Request::new(&rc, auth.clone(), ca, signed(&f, 2)).unwrap();
    let original = Message::parse(client.bytes(), 65536).unwrap();
    let Begin::Quote(quoted) = server
        .begin(
            &pc,
            &lc,
            sa,
            &original,
            Time {
                now_milliseconds: 10,
                expires_milliseconds: 100,
            },
        )
        .unwrap()
    else {
        panic!("expected new quote")
    };
    assert_eq!(quoted.association(), sa);
    assert_eq!(f.ledger.balance(&lc, f.owner), Ok(100));
    let offer = client.quote(&ic, ca, &message(quoted.record())).unwrap();
    assert_eq!(offer.fee(), 2);
    assert_eq!(offer.id(), f.order.transfer_id);
    let accept = client.accept(&rc, &offer).unwrap();
    assert_eq!(f.ledger.balance(&lc, f.owner), Ok(100));
    assert!(client.accept(&rc, &offer).is_err());
    let result = server
        .accept(&pc, &lc, &quoted, sa, &message(&accept), 20)
        .unwrap();
    assert!(
        server
            .accept(&pc, &lc, &quoted, sa, &message(&accept), 20)
            .is_err()
    );
    let completed = client.finish(&ic, ca, &message(&result), true).unwrap();
    assert_eq!(completed.accepted_fee(), Some(2));
    assert_eq!(completed.proof().result().received(), Ok(7));
    assert_eq!(completed.into_proof().issuer(), f.local);
    assert_eq!(f.ledger.balance(&lc, f.owner), Ok(91));
    assert_eq!(f.ledger.balance(&lc, f.recipient), Ok(7));
    policy.fee.store(99, Ordering::SeqCst);
    let (ca, sa) = route(&f, 5);
    let client = Request::new(&rc, auth.clone(), ca, signed(&f, 0)).unwrap();
    let Begin::Stored(result) = server
        .begin(
            &pc,
            &lc,
            sa,
            &Message::parse(client.bytes(), 65536).unwrap(),
            Time {
                now_milliseconds: 1000,
                expires_milliseconds: 1001,
            },
        )
        .unwrap()
    else {
        panic!("expected stored retry")
    };
    let observation = client.finish(&ic, ca, &message(&result), true).unwrap();
    assert_eq!(observation.accepted_fee(), None);
    assert_eq!(observation.proof().result().fee, 2);
    assert_eq!(policy.calls.load(Ordering::SeqCst), 1);
    assert_eq!(f.ledger.balance(&lc, f.owner), Ok(91));
}
// Wrong fee/ID/stream, expiration and exhausted available balance cannot create issuer commitment or alter accounts.
#[test]
fn invalid_and_unfunded_transfer_acceptance_has_no_effect() {
    for case in 0..6 {
        let f = Issuer::new();
        let server = server(&f, Arc::new(transfer::Fixed(2)));
        let auth = Arc::new(Authority::new());
        let rg = auth.grant(f.owner, Action::Request, 16_777_216).unwrap();
        let rc = context(&rg);
        let pg = f.peer.grant(f.local, Action::Invoke, 16_777_216).unwrap();
        let pc = context(&pg);
        let lg = f
            .authority
            .grant(f.local, ledger::authority::Action::Control, 16_777_216)
            .unwrap();
        let lc = ledger_context(&lg);
        let (ca, mut sa) = route(&f, 3);
        let client = Request::new(&rc, auth, ca, signed(&f, 2)).unwrap();
        let Begin::Quote(quote) = server
            .begin(
                &pc,
                &lc,
                sa,
                &Message::parse(client.bytes(), 65536).unwrap(),
                Time {
                    now_milliseconds: 0,
                    expires_milliseconds: 100,
                },
            )
            .unwrap()
        else {
            panic!("expected quote")
        };
        let mut v = serde_json::json!({"type":"ledger.transfer.accept","transferId":super::base64::encode(&f.order.transfer_id),"fee":"2"});
        let mut now = 20;
        match case {
            0 => v["fee"] = serde_json::json!("3"),
            1 => v["transferId"] = serde_json::json!(super::base64::encode(&[2; 16])),
            2 => sa.operation.stream += 2,
            3 => sa.operation.connection = [0; 32],
            4 => now = 100,
            _ => {
                let admin = f
                    .authority
                    .grant(f.local, ledger::authority::Action::Administer, 16_777_216)
                    .unwrap();
                f.ledger
                    .revoke(&ledger_context(&admin), f.owner, 100)
                    .unwrap();
            }
        }
        assert!(
            server
                .accept(
                    &pc,
                    &lc,
                    &quote,
                    sa,
                    &Message::parse(v.to_string().as_bytes(), 65536).unwrap(),
                    now
                )
                .is_err()
        );
        assert_eq!(
            f.ledger.balance(&lc, f.owner),
            Ok(if case == 5 { 0 } else { 100 })
        );
        assert_eq!(f.ledger.balance(&lc, f.recipient), Ok(0));
        let (_, original) = route(&f, 3);
        server.discard(&pc, &lc, original).unwrap();
    }
}
// Invalid signatures or a fee above the explicit ceiling fail before any quote/commit; interruption can conceal a previously stored transfer.
#[test]
fn transfer_authentication_fee_bounds_and_interruption() {
    let f = Issuer::new();
    let server = server(&f, Arc::new(transfer::Fixed(2)));
    let auth = Arc::new(Authority::new());
    let rg = auth.grant(f.owner, Action::Request, 16_777_216).unwrap();
    let rc = context(&rg);
    let (ca, sa) = route(&f, 3);
    let pg = f.peer.grant(f.local, Action::Invoke, 16_777_216).unwrap();
    let pc = context(&pg);
    let lg = f
        .authority
        .grant(f.local, ledger::authority::Action::Control, 16_777_216)
        .unwrap();
    let lc = ledger_context(&lg);
    let client = Request::new(&rc, auth.clone(), ca, signed(&f, 1)).unwrap();
    let request = Message::parse(client.bytes(), 65536).unwrap();
    assert!(matches!(
        server.begin(
            &pc,
            &lc,
            sa,
            &request,
            Time {
                now_milliseconds: 0,
                expires_milliseconds: 100
            }
        ),
        Err(Error::Insufficient)
    ));
    let mut bad = request.value().clone();
    bad["signature"] = serde_json::json!(super::base64::encode(&[0; 64]));
    assert!(
        server
            .begin(
                &pc,
                &lc,
                sa,
                &Message::parse(bad.to_string().as_bytes(), 65536).unwrap(),
                Time {
                    now_milliseconds: 0,
                    expires_milliseconds: 100
                }
            )
            .is_err()
    );
    assert_eq!(f.ledger.balance(&lc, f.owner), Ok(100));
    let mut ordering = Exchange::new(Direction::Local);
    ordering
        .control(Direction::Local, Kind::LedgerTransfer)
        .unwrap();
    assert!(!ordering.accepted());
    assert_eq!(ordering.close(false), Ok(Outcome::Unknown));
}
// Zero fee is valid and still requires explicit consent; foreign/over-limit fee offers never authorize acceptance.
#[test]
fn zero_fee_and_requester_quote_refusal() {
    let f = Issuer::new();
    let server = server(&f, Arc::new(transfer::Fixed(0)));
    let (ca, sa) = route(&f, 3);
    let auth = Arc::new(Authority::new());
    let rg = auth.grant(f.owner, Action::Request, 16_777_216).unwrap();
    let rc = context(&rg);
    let ig = auth.grant(f.owner, Action::Invoke, 16_777_216).unwrap();
    let ic = context(&ig);
    for case in 0..3 {
        let mut client = Request::new(&rc, auth.clone(), ca, signed(&f, 0)).unwrap();
        let mut offer = serde_json::json!({"type":"ledger.transfer.quote","transferId":super::base64::encode(&f.order.transfer_id),"fee":"0"});
        let mut observed = ca;
        match case {
            0 => offer["fee"] = serde_json::json!("1"),
            1 => offer["transferId"] = serde_json::json!(super::base64::encode(&[2; 16])),
            _ => observed.operation.stream += 2,
        }
        assert!(
            client
                .quote(
                    &ic,
                    observed,
                    &Message::parse(offer.to_string().as_bytes(), 65536).unwrap()
                )
                .is_err()
        );
    }
    let mut client = Request::new(&rc, auth.clone(), ca, signed(&f, 0)).unwrap();
    let pg = f.peer.grant(f.local, Action::Invoke, 16_777_216).unwrap();
    let pc = context(&pg);
    let lg = f
        .authority
        .grant(f.local, ledger::authority::Action::Control, 16_777_216)
        .unwrap();
    let lc = ledger_context(&lg);
    let Begin::Quote(quote) = server
        .begin(
            &pc,
            &lc,
            sa,
            &Message::parse(client.bytes(), 65536).unwrap(),
            Time {
                now_milliseconds: 0,
                expires_milliseconds: 100,
            },
        )
        .unwrap()
    else {
        panic!("expected quote")
    };
    let offer = client.quote(&ic, ca, &message(quote.record())).unwrap();
    assert_eq!(offer.fee(), 0);
    let accept = client.accept(&rc, &offer).unwrap();
    let result = server
        .accept(&pc, &lc, &quote, sa, &message(&accept), 1)
        .unwrap();
    let complete = client.finish(&ic, ca, &message(&result), true).unwrap();
    assert_eq!(complete.accepted_fee(), Some(0));
    assert_eq!(complete.proof().result().received(), Ok(9));
    assert_eq!(f.ledger.balance(&lc, f.owner), Ok(91));
    assert_eq!(f.ledger.balance(&lc, f.recipient), Ok(9));
}

// A valid fee quote with an undersized local result node budget cannot move any credits.
#[test]
fn response_capacity_is_checked_before_transfer_commit() {
    let f = Issuer::new();
    let server = transfer::Issuer::new(transfer::Config {
        peer: f.peer.clone(),
        authority: f.authority.clone(),
        ledger: f.ledger.clone(),
        limits: Limits {
            nodes: 4,
            ..Default::default()
        },
        policy: Arc::new(transfer::Fixed(2)),
    })
    .unwrap();
    let auth = Arc::new(Authority::new());
    let grant = auth.grant(f.owner, Action::Request, 16_777_216).unwrap();
    let pg = f.peer.grant(f.local, Action::Invoke, 16_777_216).unwrap();
    let lg = f
        .authority
        .grant(f.local, ledger::authority::Action::Control, 16_777_216)
        .unwrap();
    let lc = ledger_context(&lg);
    let (ca, sa) = route(&f, 1);
    let client = Request::new(&context(&grant), auth, ca, signed(&f, 2)).unwrap();
    let Begin::Quote(quote) = server
        .begin(
            &context(&pg),
            &lc,
            sa,
            &Message::parse(client.bytes(), 65536).unwrap(),
            Time {
                now_milliseconds: 0,
                expires_milliseconds: 100,
            },
        )
        .unwrap()
    else {
        panic!("new quote")
    };
    let accept = Message::parse(serde_json::json!({"type":"ledger.transfer.accept", "transferId": super::base64::encode(&f.order.transfer_id), "fee":"2"}).to_string().as_bytes(),65536).unwrap();
    assert_eq!(
        server.accept(&context(&pg), &lc, &quote, sa, &accept, 1),
        Err(Error::Limit)
    );
    assert_eq!(f.ledger.balance(&lc, f.owner), Ok(100));
    assert_eq!(f.ledger.balance(&lc, f.recipient), Ok(0));
}
