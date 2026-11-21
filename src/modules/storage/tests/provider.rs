//! Published Storage provider integration through actual Board, Services, Ledger and native Storage.
use super::*;
use crate::modules::{
    board, ledger,
    storage::{
        bytes::{Store, tests::path},
        contract::Time,
        journal::Journal,
        requester::client::{Client, Policy},
        retention::Anchor,
        standard::Catalogue,
        test_support::{
            contracts,
            mutations::{B, CryptoPort, image_limits, limits, time},
        },
    },
};
use services::handler::Provider as _;
use services::{
    authority::{Action, Authority},
    contract::Context,
    quote::{BoardCurrent, Issue, Quoter},
    runtime::{Config, Coordinator, Decision, Payment},
};
use std::{
    collections::BTreeMap,
    sync::Mutex,
    time::{Duration, Instant},
};

struct TestClock(Mutex<Time>);
impl Clock for TestClock {
    // Return a deterministic observation that tests may move between preparation and decision selection.
    fn now(&self) -> Result<Time, Error> {
        Ok(*self.0.lock().unwrap())
    }
}

// Use an actual published contract for all four operations and verify one provider-side charge per commitment.
#[test]
fn published_storage_runs_all_operations_through_services() {
    let dir = path();
    let ledger_dir = path();
    let services_dir = path();
    let bytes = Arc::new(Store::open(&dir.0, true).unwrap());
    let (fixture, store_input, _) = contracts::store(false);
    let (local, remote) = (fixture.provider, fixture.requester);
    let crypto = Arc::new(CryptoPort);
    let journal = Arc::new(
        Journal::initialize(
            local,
            bytes.clone(),
            crypto.clone(),
            image_limits(),
            Anchor::new(time(0)),
        )
        .unwrap(),
    );
    let gate = Arc::new(Gate::new(journal.clone(), bytes, limits()).unwrap());
    let clock = Arc::new(TestClock(Mutex::new(time(1))));
    let provider = Arc::new(
        Provider::new(
            gate.clone(),
            crypto,
            clock.clone(),
            retrieve_result::Limits {
                result_bytes: 4096,
                object_bytes: 64,
                values: B,
            },
            8,
        )
        .unwrap(),
    );
    let catalogue = Arc::new(Catalogue::new(Default::default()).unwrap());
    let family = Arc::new(
        Client::new(
            fixture.definition.clone(),
            catalogue.as_ref(),
            provider.clone(),
            clock.clone(),
            Policy {
                values: B,
                maximum_clock_difference_seconds: 60,
            },
        )
        .unwrap(),
    );
    let authority = Arc::new(Authority::new());
    let registry = Arc::new(
        services::registry::Registry::new(local, authority.clone(), Default::default(), catalogue)
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(60);
    let register = authority.grant(local, Action::Register, 1_000_000).unwrap();
    let context = Context {
        version: 1,
        grant: &register,
        deadline,
        cancelled: false,
    };
    registry
        .register(&context, family, Some(provider.clone()))
        .unwrap();

    let ledger_auth = Arc::new(ledger::authority::Authority::new());
    let admin = ledger_auth
        .grant(local, ledger::authority::Action::Administer, 1_000_000)
        .unwrap();
    let bank_context = ledger::contract::Context {
        version: 1,
        grant: &admin,
        deadline,
        cancelled: false,
    };
    let bank = Arc::new(
        ledger::state::Ledger::initialize(
            local,
            ledger_auth.clone(),
            ledger::linux::FileStore::open(&ledger_dir.0, Default::default(), true).unwrap(),
            Default::default(),
            &bank_context,
            1000,
            100,
        )
        .unwrap(),
    );
    bank.grant(&bank_context, remote, 100).unwrap();
    bank.set_trust(
        &bank_context,
        remote,
        ledger::accounting::Trust::new(1).unwrap(),
    )
    .unwrap();
    let bank_coordinate = ledger_auth
        .grant(local, ledger::authority::Action::Coordinate, 1_000_000)
        .unwrap();
    let accounting = Arc::new(services::accounting::Ledger::new(
        bank.clone(),
        bank_coordinate,
        Default::default(),
    ));
    let runtime = Coordinator::open(
        Config {
            local,
            authority: authority.clone(),
            limits: Default::default(),
        },
        &context,
        services::linux::FileStore::open(&services_dir.0, Default::default(), true).unwrap(),
        accounting,
        registry.clone(),
        true,
    )
    .unwrap();
    let coordinate = authority
        .grant(local, Action::Coordinate, 1_000_000)
        .unwrap();
    let cc = Context {
        grant: &coordinate,
        ..context
    };
    let invoke = authority.grant(local, Action::Invoke, 1_000_000).unwrap();
    let ic = Context {
        grant: &invoke,
        ..context
    };
    let observe = authority.grant(local, Action::Observe, 1_000_000).unwrap();
    let oc = Context {
        grant: &observe,
        ..context
    };
    gate.recover(
        &crate::modules::storage::recovery::Services {
            coordinator: &runtime,
            context: &cc,
        },
        time(1),
        None,
    )
    .unwrap();

    let board_auth = Arc::new(board::authority::Authority::new());
    let board = board::state::Provider::new(
        local,
        board_auth.clone(),
        Default::default(),
        registry.clone(),
    )
    .unwrap();
    let publish = board_auth
        .grant(local, board::authority::Action::Publish, 1_000_000)
        .unwrap();
    let select = board_auth
        .grant(local, board::authority::Action::Select, 1_000_000)
        .unwrap();
    let bp = board::contract::Context {
        grant: &publish,
        deadline,
        cancelled: false,
    };
    let bs = board::contract::Context {
        grant: &select,
        ..bp
    };
    let mut offering = fixture.offering.source.clone();
    offering["creditIssuer"] = crate::modules::identity::text::format(local).into();
    let raw = serde_json::json!({"version":1,"providedServices":[offering],"wantedServices":[]});
    let id = &fixture.offering.id;
    let prepared = board::prepared::Prepared::new(
        local,
        &serde_json::to_vec(&raw).unwrap(),
        &BTreeMap::from([(id.clone(), fixture.definition.document.bytes().to_vec())]),
        Default::default(),
        registry.as_ref(),
    )
    .unwrap();
    let snapshot = board.publish(&bp, prepared).unwrap();
    let selection = board
        .select(
            &bs,
            &snapshot,
            &board::selection::Request {
                id,
                service: "cr2se.storage",
                version: 1,
                description: &fixture.offering.description,
                other: remote,
                authenticated: true,
            },
        )
        .unwrap();
    let quoter = Quoter::new(local, authority.clone(), Default::default()).unwrap();
    let lease_id = Value::Bytes(vec![7; 32]);
    let inputs = [
        (store_input, 17),
        (
            Value::Object(vec![
                ("operation".into(), Value::String("retrieve".into())),
                ("leaseId".into(), lease_id.clone()),
                ("offset".into(), Value::Uint64(0)),
                ("length".into(), Value::Uint64(3)),
            ]),
            2,
        ),
        (
            Value::Object(vec![
                ("operation".into(), Value::String("renew".into())),
                ("leaseId".into(), lease_id.clone()),
                ("expectedExpiresAt".into(), Value::Uint64(1030)),
                ("additionalSeconds".into(), Value::Uint64(4)),
            ]),
            6,
        ),
        (
            Value::Object(vec![
                ("operation".into(), Value::String("remove".into())),
                ("leaseId".into(), lease_id),
            ]),
            2,
        ),
    ];
    for (index, (input, price)) in inputs.into_iter().enumerate() {
        let key = OperationKey {
            stream: 1 + u32::try_from(index).unwrap() * 2,
            ..fixture.key
        };
        let peer = Peer { local, remote, key };
        let authorized = services::authorization::authorize(
            &ic,
            &authority,
            local,
            &registry,
            selection.clone(),
            peer,
            None,
        )
        .unwrap();
        let assessed =
            services::assessment::assess(&ic, &authority, authorized, &input, Default::default())
                .unwrap();
        let quote = quoter
            .issue(
                &ic,
                Issue {
                    assessed,
                    bytes: Blob::new(vec![index as u8], 1).unwrap(),
                    maximum: price,
                    expires: deadline,
                },
            )
            .unwrap();
        assert_eq!(quote.price(), services::pricing::Price::Fixed(price));
        let accepted = quoter
            .accept(
                &ic,
                &quote,
                peer,
                quote.hash(),
                &BoardCurrent {
                    owner: &board,
                    context: &bs,
                },
            )
            .unwrap();
        assert_eq!(
            runtime.admit(&cc, accepted, Payment::default()).unwrap(),
            key
        );
        runtime.supply_input(&cc, key, input.clone()).unwrap();
        runtime.start(&cc, key).unwrap();
        runtime.stage(&cc, key).unwrap();
        runtime.prepare(&cc, key).unwrap();
        if index == 0 {
            assert!(journal.snapshot().unwrap().leases.is_empty());
            *clock.0.lock().unwrap() = time(20);
        }
        runtime.decide(&cc, key, Decision::Commit).unwrap();
        runtime.commit(&cc, key).unwrap();
        runtime.commit(&cc, key).unwrap();
        let result = runtime.result(&oc, key).unwrap();
        assert_eq!(result.price, price);
        if index == 0 {
            assert_eq!(renewal::number(&result.output, "acceptedAt"), Ok(1020));
            assert_eq!(renewal::number(&result.output, "expiresAt"), Ok(1030));
        }
        if index == 1 {
            assert!(
                matches!(&result.output, Value::Object(fields) if fields.contains(&("data".into(), Value::Bytes(b"abc".to_vec()))))
            );
        }
        if index == 3 {
            assert!(matches!(
                provider.assess(peer, selection.offering(), &input),
                Err(Error::Unavailable)
            ));
        }
    }
    let read = ledger_auth
        .grant(local, ledger::authority::Action::Read, 1_000_000)
        .unwrap();
    assert_eq!(
        bank.view(
            &ledger::contract::Context {
                grant: &read,
                ..bank_context
            },
            remote
        )
        .unwrap()
        .issued,
        73
    );
    assert_eq!(
        journal.snapshot().unwrap().leases[0].state,
        crate::modules::storage::contract::State::Removed
    );
}
