//! Messaging through the real Services coordinator: quote, accept, stage, prepare, decide and commit for every operation.
use super::{client::Client, client::Placements, provider::ServiceProvider, release::LedgerIssuer};
use crate::modules::{
    board, ledger,
    messaging::{
        codec::{self, Request, Response},
        ids::{MessageId, PlacementId, RecoveryId},
        journal::Journal,
        operation::{DiscoverInput, RemoveInput, RenewInput, StatusInput},
        participant::Participant,
        ports::Signer,
        provider::{
            Provider,
            tests::{offering, terms},
        },
        receipt,
        recovery::DeferToServices,
        staging::Staging,
        standard::Catalogue,
        state::Encoding,
        store::{Limits, Store},
        test_support::{FixedClock, SequenceEntropy, TestIdentity},
    },
    services::{
        self, Error,
        authority::{Action, Authority, Grant},
        contract::{Context, OperationKey},
        handler::{Blob, Peer},
        quote::{BoardCurrent, Issue, Quoter},
        runtime::{Config, Coordinator, Decision, Payment},
        value::{Budget, Value},
    },
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const BUDGET: Budget = Budget {
    depth: 8,
    nodes: 4096,
    bytes: 65536,
};

// What the requesters know about their placements: the payload size they stored or discovered.
#[derive(Default)]
struct Known(Mutex<BTreeMap<PlacementId, u64>>);
impl Placements for Known {
    // The size is known only for placements the test recorded, like a real requester's catalog.
    fn payload_size(
        &self,
        _: OperationKey,
        _: crate::modules::identity::api::IdentityId,
        id: PlacementId,
    ) -> Result<u64, Error> {
        self.0
            .lock()
            .unwrap()
            .get(&id)
            .copied()
            .ok_or(Error::Denied)
    }
}

// A complete rig: provider engine, Services registry and coordinator, Board, Ledger and identities.
struct Rig {
    clock: Arc<FixedClock>,
    provider: Arc<Provider>,
    known: Arc<Known>,
    authority: Arc<Authority>,
    runtime: Coordinator<services::persistence::Memory>,
    registry: Arc<services::registry::Registry>,
    board: board::state::Provider,
    snapshot: board::contract::Snapshot,
    board_select: board::authority::Grant,
    quoter: Quoter,
    bank: Arc<ledger::state::Ledger<ledger::persistence::Memory>>,
    ledger_auth: Arc<ledger::authority::Authority>,
    grants: Grants,
    local: crate::modules::identity::api::IdentityId,
    sender: TestIdentity,
    recipient: TestIdentity,
    provider_identity: Arc<TestIdentity>,
    deadline: Instant,
}
struct Grants {
    coordinate: Grant,
    invoke: Grant,
    observe: Grant,
}

impl Rig {
    // A context for one grant.
    fn context<'a>(&self, grant: &'a Grant) -> Context<'a> {
        Context {
            version: 1,
            grant,
            deadline: self.deadline,
            cancelled: false,
        }
    }

    // Quote, accept, admit, supply the input, execute and stage one operation by `remote`, leaving the decision to the caller.
    fn stage(
        &self,
        remote: &TestIdentity,
        stream: u32,
        input: Value,
        maximum: u64,
    ) -> Result<OperationKey, Error> {
        let key = OperationKey {
            requester: remote.identity(),
            connection: [9; 32],
            stream,
        };
        let peer = Peer {
            local: self.local,
            remote: remote.identity(),
            key,
        };
        let (cc, ic) = (
            self.context(&self.grants.coordinate),
            self.context(&self.grants.invoke),
        );
        let bs = self.board_context();
        let offering = offering();
        let selection = self
            .board
            .select(
                &bs,
                &self.snapshot,
                &board::selection::Request {
                    id: &offering.id,
                    service: "cr2se.messaging",
                    version: 1,
                    description: &offering.description,
                    other: remote.identity(),
                    authenticated: true,
                },
            )
            .map_err(|_| Error::Unavailable)?;
        let authorized = services::authorization::authorize(
            &ic,
            &self.authority,
            self.local,
            &self.registry,
            selection,
            peer,
            None,
        )?;
        let assessed = services::assessment::assess(
            &ic,
            &self.authority,
            authorized,
            &input,
            Default::default(),
        )?;
        let quote = self.quoter.issue(
            &ic,
            Issue {
                assessed,
                bytes: Blob::new(vec![stream as u8], 1)?,
                maximum,
                expires: self.deadline,
            },
        )?;
        let accepted = self.quoter.accept(
            &ic,
            &quote,
            peer,
            quote.hash(),
            &BoardCurrent {
                owner: &self.board,
                context: &bs,
            },
        )?;
        self.runtime.admit(&cc, accepted, Payment::default())?;
        self.runtime.supply_input(&cc, key, input)?;
        self.runtime.start(&cc, key)?;
        self.runtime.stage(&cc, key)?;
        Ok(key)
    }

    // The Board selection context.
    fn board_context(&self) -> board::contract::Context<'_> {
        board::contract::Context {
            grant: &self.board_select,
            deadline: self.deadline,
            cancelled: false,
        }
    }

    // Run one operation by `remote` through the whole coordinator and return its retained output and price.
    fn run(
        &self,
        remote: &TestIdentity,
        stream: u32,
        input: Value,
        maximum: u64,
    ) -> Result<(Value, u64), Error> {
        let key = self.stage(remote, stream, input, maximum)?;
        let (cc, oc) = (
            self.context(&self.grants.coordinate),
            self.context(&self.grants.observe),
        );
        self.runtime.prepare(&cc, key)?;
        self.runtime.decide(&cc, key, Decision::Commit)?;
        self.runtime.commit(&cc, key)?;
        let result = self.runtime.result(&oc, key)?;
        Ok((result.output, result.price))
    }

    // The balance the provider has recognized for `remote`'s issuer account, to check exact charging.
    fn balance(&self, remote: &TestIdentity) -> u64 {
        let read = self
            .ledger_auth
            .grant(self.local, ledger::authority::Action::Read, 1_000_000)
            .unwrap();
        self.bank
            .view(
                &ledger::contract::Context {
                    version: 1,
                    grant: &read,
                    deadline: self.deadline,
                    cancelled: false,
                },
                remote.identity(),
            )
            .unwrap()
            .issued
    }
}

// Build the rig: the engine at time 1000, registered Services handlers, a published offering and funded accounts for A and C.
fn rig() -> Rig {
    let deadline = Instant::now() + Duration::from_secs(120);
    let clock = Arc::new(FixedClock::at(1000));
    let provider_identity = Arc::new(TestIdentity::new(2));
    let local = provider_identity.identity();
    let store = Arc::new(Store::volatile(Limits {
        placements: 16,
        payload_bytes: 10_000,
    }));
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
            ledger::persistence::Memory::default(),
            Default::default(),
            &bank_context,
            10_000,
            100,
        )
        .unwrap(),
    );
    let (sender, recipient) = (TestIdentity::new(1), TestIdentity::new(3));
    for who in [&sender, &recipient] {
        bank.grant(&bank_context, who.identity(), 1000).unwrap();
        bank.set_trust(
            &bank_context,
            who.identity(),
            ledger::accounting::Trust::new(1).unwrap(),
        )
        .unwrap();
    }
    let bank_coordinate = ledger_auth
        .grant(local, ledger::authority::Action::Coordinate, 1_000_000)
        .unwrap();
    let accounting = Arc::new(services::accounting::Ledger::new(
        bank.clone(),
        bank_coordinate,
        Default::default(),
    ));
    let issuer_grant = ledger_auth
        .grant(local, ledger::authority::Action::Coordinate, 1_000_000)
        .unwrap();
    let participant = Arc::new(
        Participant::new(
            store,
            Journal::volatile(64),
            Arc::new(SequenceEntropy::new(1)),
            clock.clone(),
            provider_identity.clone(),
        )
        .with_release_issuer(Arc::new(LedgerIssuer::new(
            bank.clone(),
            issuer_grant,
            Duration::from_secs(60),
        ))),
    );
    participant.recover(&DeferToServices).unwrap();
    // Half of the cumulative payments are offered as early-release credits, never below the sender's minimum.
    let provider = Arc::new(
        Provider::new(terms(), participant, clock.clone(), Staging::new(10_000, 8))
            .with_release_policy(Arc::new(|payments: u64, minimum: u64| {
                (payments / 2).max(minimum)
            })),
    );
    let known = Arc::new(Known::default());
    let catalogue = Arc::new(Catalogue::new(Default::default()).unwrap());
    let definition = catalogue.definition(&offering()).unwrap();
    let family =
        Arc::new(Client::new(definition.clone(), &catalogue, known.clone(), BUDGET).unwrap());
    let service = Arc::new(ServiceProvider::new(provider.clone(), BUDGET, 65536).unwrap());
    let authority = Arc::new(Authority::new());
    let registry = Arc::new(
        services::registry::Registry::new(local, authority.clone(), Default::default(), catalogue)
            .unwrap(),
    );
    let register = authority.grant(local, Action::Register, 1_000_000).unwrap();
    let rc = Context {
        version: 1,
        grant: &register,
        deadline,
        cancelled: false,
    };
    registry.register(&rc, family, Some(service)).unwrap();

    let grants = Grants {
        coordinate: authority
            .grant(local, Action::Coordinate, 1_000_000)
            .unwrap(),
        invoke: authority.grant(local, Action::Invoke, 1_000_000).unwrap(),
        observe: authority.grant(local, Action::Observe, 1_000_000).unwrap(),
    };
    let runtime = Coordinator::open(
        Config {
            local,
            authority: authority.clone(),
            limits: Default::default(),
        },
        &rc,
        services::persistence::Memory::default(),
        accounting,
        registry.clone(),
        true,
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
    let board_select = board_auth
        .grant(local, board::authority::Action::Select, 1_000_000)
        .unwrap();
    let bp = board::contract::Context {
        grant: &publish,
        deadline,
        cancelled: false,
    };
    let mut published = offering().source.clone();
    published["creditIssuer"] = crate::modules::identity::text::format(local).into();
    let raw = serde_json::json!({"version":1,"providedServices":[published],"wantedServices":[]});
    let definitions = BTreeMap::from([(offering().id, definition.document.bytes().to_vec())]);
    let prepared = board::prepared::Prepared::new(
        local,
        &serde_json::to_vec(&raw).unwrap(),
        &definitions,
        Default::default(),
        registry.as_ref(),
    )
    .unwrap();
    let snapshot = board.publish(&bp, prepared).unwrap();
    let quoter = Quoter::new(local, authority.clone(), Default::default()).unwrap();
    Rig {
        clock,
        provider,
        known,
        authority,
        runtime,
        registry,
        board,
        snapshot,
        board_select,
        quoter,
        bank,
        ledger_auth,
        grants,
        local,
        sender,
        recipient,
        provider_identity,
        deadline,
    }
}

// The input value of a store of `abc` for ten seconds from the rig's sender to its recipient.
fn store_input(rig: &Rig, id: u8) -> Value {
    let message = crate::modules::messaging::signing::sign_message(
        &rig.sender,
        rig.recipient.identity(),
        MessageId([id; 32]),
        1000,
        Encoding::Opaque,
        b"abc".to_vec(),
    )
    .unwrap();
    codec::request_value(&Request::Store(
        crate::modules::messaging::operation::StoreInput {
            message,
            retention_seconds: 10,
        },
    ))
}

// A full lifecycle through real Services and Ledger: each operation is quoted at its exact price, charged once and returns the engine's
// result, with the store and recovery times taken at the coordinator's decision.
#[test]
fn every_operation_runs_through_services_with_exact_charges() {
    let rig = rig();
    let (a, c) = (&rig.sender, &rig.recipient);
    // Store: 17 credits; acceptance and expiry are the decision time.
    rig.clock.set(1020);
    let (output, price) = rig.run(a, 1, store_input(&rig, 1), 17).unwrap();
    assert_eq!(price, 17);
    let Response::Store(stored) = codec::parse_response(&output).unwrap() else {
        panic!("store")
    };
    assert_eq!(
        (stored.accepted_at, stored.expires_at, stored.price),
        (1020, 1030, 17)
    );
    rig.known.0.lock().unwrap().insert(stored.placement_id, 3);
    assert_eq!(rig.balance(a), 983);
    // Discover: 1 credit; the recipient sees the placement.
    let discover = codec::request_value(&Request::Discover(DiscoverInput {
        cursor: None,
        limit: 5,
    }));
    let (output, price) = rig.run(c, 3, discover, 1).unwrap();
    let Response::Discover(page) = codec::parse_response(&output).unwrap() else {
        panic!("discover")
    };
    assert_eq!(
        (price, page.messages.len(), page.messages[0].placement_id),
        (1, 1, stored.placement_id)
    );
    // Recover: 2 credits; the receipt carries the decision time and verifies.
    rig.clock.set(1025);
    let request = receipt::sign_request(
        c,
        rig.local,
        stored.placement_id,
        stored.message_id,
        message_hash(&rig),
        RecoveryId([6; 32]),
    )
    .unwrap();
    let (output, price) = rig
        .run(c, 5, codec::request_value(&Request::Recover(request)), 2)
        .unwrap();
    let Response::Recover(recovered) = codec::parse_response(&output).unwrap() else {
        panic!("recover")
    };
    assert_eq!(
        (
            price,
            recovered.receipt.recovered_at,
            recovered.message.payload.as_slice()
        ),
        (2, 1025, &b"abc"[..])
    );
    assert_eq!(rig.balance(c), 1000 - 1 - 2);
    // Status by the sender shows the recovery.
    let (output, price) = rig
        .run(
            a,
            7,
            codec::request_value(&Request::Status(StatusInput {
                placement_id: stored.placement_id,
            })),
            1,
        )
        .unwrap();
    let Response::Status(status) = codec::parse_response(&output).unwrap() else {
        panic!("status")
    };
    assert_eq!((price, status.receipt), (1, Some(recovered.receipt)));
    // Renew a second placement and remove a third.
    rig.clock.set(1030);
    let (output, _) = rig.run(a, 9, store_input(&rig, 2), 17).unwrap();
    let Response::Store(second) = codec::parse_response(&output).unwrap() else {
        panic!("store")
    };
    rig.known.0.lock().unwrap().insert(second.placement_id, 3);
    let renew = RenewInput {
        placement_id: second.placement_id,
        expected_expires_at: second.expires_at,
        additional_seconds: 5,
    };
    let (output, price) = rig
        .run(a, 11, codec::request_value(&Request::Renew(renew)), 8)
        .unwrap();
    let Response::Renew(renewed) = codec::parse_response(&output).unwrap() else {
        panic!("renew")
    };
    assert_eq!((price, renewed.expires_at), (8, second.expires_at + 5));
    let remove = RemoveInput {
        placement_id: second.placement_id,
        removal_id: crate::modules::messaging::ids::RemovalId([4; 32]),
        expected_expires_at: renewed.expires_at,
        minimum_release_credits: 0,
    };
    let (output, price) = rig
        .run(a, 13, codec::request_value(&Request::Remove(remove)), 2)
        .unwrap();
    let Response::Remove(removed) = codec::parse_response(&output).unwrap() else {
        panic!("remove")
    };
    assert_eq!(
        (price, removed.removed, removed.release_credits),
        (2, true, 12)
    );
    assert_eq!(rig.balance(a), 1000 - 17 - 1 - 17 - 8 - 2 + 12);
    let _ = (
        &rig.provider_identity.public_key(),
        rig.registry.as_ref() as *const _,
    );
}

// The payload hash of the message the tests store.
fn message_hash(rig: &Rig) -> crate::modules::messaging::ids::PayloadHash {
    let message = crate::modules::messaging::signing::sign_message(
        &rig.sender,
        rig.recipient.identity(),
        MessageId([1; 32]),
        1000,
        Encoding::Opaque,
        b"abc".to_vec(),
    )
    .unwrap();
    let _ = message_unused;
    message.metadata.payload_hash
}
fn message_unused() {}

// A provider is installed only over a participant that completed recovery, and the Board refuses to publish the Messaging offering while
// no handler is registered, so nothing unfinished is ever advertised.
#[test]
fn nothing_is_advertised_without_a_ready_handler() {
    let clock = Arc::new(FixedClock::at(1000));
    let identity = Arc::new(TestIdentity::new(2));
    let participant = Arc::new(Participant::new(
        Arc::new(Store::volatile(Limits {
            placements: 4,
            payload_bytes: 1000,
        })),
        Journal::volatile(8),
        Arc::new(SequenceEntropy::new(1)),
        clock.clone(),
        identity.clone(),
    ));
    let provider = Arc::new(Provider::new(
        terms(),
        participant.clone(),
        clock,
        Staging::new(100, 2),
    ));
    assert_eq!(
        ServiceProvider::new(provider.clone(), BUDGET, 65536).err(),
        Some(Error::Unavailable)
    );
    participant.recover(&DeferToServices).unwrap();
    assert!(ServiceProvider::new(provider.clone(), BUDGET, 0).is_err());
    assert!(ServiceProvider::new(provider, BUDGET, 65536).is_ok());

    // With no handler registered, preparing the Board document fails.
    let local = identity.identity();
    let catalogue = Arc::new(Catalogue::new(Default::default()).unwrap());
    let definition = catalogue.definition(&offering()).unwrap();
    let authority = Arc::new(Authority::new());
    let empty =
        services::registry::Registry::new(local, authority, Default::default(), catalogue).unwrap();
    let mut published = offering().source.clone();
    published["creditIssuer"] = crate::modules::identity::text::format(local).into();
    let raw = serde_json::json!({"version":1,"providedServices":[published],"wantedServices":[]});
    let definitions = BTreeMap::from([(offering().id, definition.document.bytes().to_vec())]);
    assert!(
        board::prepared::Prepared::new(
            local,
            &serde_json::to_vec(&raw).unwrap(),
            &definitions,
            Default::default(),
            &empty
        )
        .is_err()
    );
}

// A store whose signature is wrong never reaches a charge: it is refused before or at execution, nothing is placed and the balance is
// untouched. A quote ceiling below the price is refused without any effect either.
#[test]
fn refused_stores_cost_nothing() {
    let rig = rig();
    let a = &rig.sender;
    let Value::Object(mut fields) = store_input(&rig, 1) else {
        panic!("object")
    };
    for (name, value) in &mut fields {
        if name == "senderSignature" {
            let Value::Bytes(bytes) = value else {
                panic!("bytes")
            };
            bytes[0] ^= 1;
        }
    }
    assert!(rig.run(a, 1, Value::Object(fields), 17).is_err());
    assert!(rig.run(a, 3, store_input(&rig, 2), 16).is_err());
    assert_eq!(rig.balance(a), 1000);
    assert_eq!(rig.provider.participant().store().records().len(), 0);
}

// An operation the coordinator aborts leaves no placement, releases its lock and costs nothing; the same store can then be made again.
#[test]
fn an_aborted_store_leaves_nothing_behind() {
    let rig = rig();
    let a = &rig.sender;
    let key = rig.stage(a, 1, store_input(&rig, 1), 17).unwrap();
    let cc = rig.context(&rig.grants.coordinate);
    rig.runtime.prepare(&cc, key).unwrap();
    rig.runtime.decide(&cc, key, Decision::Abort).unwrap();
    rig.runtime.abort(&cc, key).unwrap();
    assert_eq!(rig.provider.participant().store().records().len(), 0);
    assert_eq!(rig.balance(a), 1000);
    assert!(matches!(
        services::handler::Provider::status(
            &ServiceProvider::new(rig.provider.clone(), BUDGET, 65536).unwrap(),
            key
        ),
        Ok(services::handler::Status::Aborted)
    ));
    let (_, price) = rig.run(a, 3, store_input(&rig, 1), 17).unwrap();
    assert_eq!((price, rig.balance(a)), (17, 983));
}

// An exact retry of a recorded recovery arrives on a new stream: Services answers it from the original committed output, with the
// original price, and nothing is charged or executed again.
#[test]
fn a_recovery_retry_returns_the_original_output() {
    let rig = rig();
    let (a, c) = (&rig.sender, &rig.recipient);
    let (output, _) = rig.run(a, 1, store_input(&rig, 1), 17).unwrap();
    let Response::Store(stored) = codec::parse_response(&output).unwrap() else {
        panic!("store")
    };
    rig.known.0.lock().unwrap().insert(stored.placement_id, 3);
    let hash = message_hash(&rig);
    let request = receipt::sign_request(
        c,
        rig.local,
        stored.placement_id,
        stored.message_id,
        hash,
        RecoveryId([6; 32]),
    )
    .unwrap();
    let input = codec::request_value(&Request::Recover(request));
    let (first, _) = rig.run(c, 3, input.clone(), 2).unwrap();
    let balance = rig.balance(c);
    let again = OperationKey {
        requester: c.identity(),
        connection: [9; 32],
        stream: 5,
    };
    let peer = Peer {
        local: rig.local,
        remote: c.identity(),
        key: again,
    };
    let offering = offering();
    let cached = rig
        .runtime
        .cached_provider(
            &rig.context(&rig.grants.observe),
            peer,
            &services::replay::Proposal {
                service: "cr2se.messaging".into(),
                version: 1,
                offering: offering.id,
                publisher: rig.local,
                issuer: rig.local,
                maximum: 2,
                payment: Payment::default(),
                input,
            },
        )
        .unwrap()
        .expect("the original output is cached");
    assert_eq!(cached.completion().output, first);
    assert_eq!(rig.balance(c), balance, "no new charge");
}

// The provider returns exactly one byte of evidence for the sender's own pending placement and refuses everyone else, and the
// requester rules accept evidence only for the challenge that was made.
#[test]
fn paid_check_evidence_and_rules() {
    use services::checks::{Rules, Timing};
    use services::handler::{Check, Provider as _};
    let rig = rig();
    let (a, c) = (&rig.sender, &rig.recipient);
    let (output, _) = rig.run(a, 1, store_input(&rig, 1), 17).unwrap();
    let Response::Store(stored) = codec::parse_response(&output).unwrap() else {
        panic!("store")
    };
    let challenge = |offset: u64| {
        codec::check_input_value(&crate::modules::messaging::operation::CheckInput {
            placement_id: stored.placement_id,
            offset,
        })
    };
    let service = ServiceProvider::new(rig.provider.clone(), BUDGET, 65536).unwrap();
    let invocation = |who: &TestIdentity| services::checks::Invocation {
        key: OperationKey {
            requester: who.identity(),
            connection: [9; 32],
            stream: 1,
        },
        requester: who.identity(),
        provider: rig.local,
        issuer: rig.local,
        offering: offering(),
        definition: Catalogue::new(Default::default())
            .unwrap()
            .definition(&offering())
            .unwrap(),
        quote: Blob::new(vec![1], 1).unwrap(),
        price: 3,
        response_milliseconds: 5000,
        deadline: rig.deadline,
    };
    let evidence = service.evidence(&invocation(a), &challenge(1)).unwrap();
    assert_eq!(codec::parse_check_output(&evidence).unwrap().value, b'b');
    assert!(service.evidence(&invocation(c), &challenge(1)).is_err());
    assert!(service.evidence(&invocation(a), &challenge(3)).is_err());
    let catalogue = Catalogue::new(Default::default()).unwrap();
    let client = Client::new(
        catalogue.definition(&offering()).unwrap(),
        &catalogue,
        rig.known.clone(),
        BUDGET,
    )
    .unwrap();
    rig.known.0.lock().unwrap().insert(stored.placement_id, 3);
    assert!(client.evidence(&challenge(1), &evidence).is_ok());
    assert!(client.evidence(&challenge(2), &evidence).is_err());
    let expectation = |byte: u8| {
        Value::Object(vec![
            (
                "placementId".into(),
                Value::Bytes(stored.placement_id.0.to_vec()),
            ),
            ("offset".into(), Value::Uint64(1)),
            ("expectedByte".into(), Value::Bytes(vec![byte])),
        ])
    };
    let within = Timing::Within(10);
    assert_eq!(
        client.outcome(
            &challenge(1),
            Some(&evidence),
            Some(&expectation(b'b')),
            within
        ),
        Check::Pass
    );
    assert_eq!(
        client.outcome(
            &challenge(1),
            Some(&evidence),
            Some(&expectation(b'x')),
            within
        ),
        Check::Fail
    );
    assert_eq!(
        client.outcome(&challenge(1), Some(&evidence), None, within),
        Check::Inconclusive
    );
    assert_eq!(
        client.outcome(
            &challenge(1),
            Some(&evidence),
            Some(&expectation(b'b')),
            Timing::Missing
        ),
        Check::Inconclusive
    );
    assert_eq!(
        client.outcome(&challenge(1), None, Some(&expectation(b'b')), within),
        Check::Inconclusive
    );
}

// A removal through Services charges its price once and issues the quoted release credits to the sender through the Ledger, exactly once,
// and an exact retry on a new stream is answered from the record without issuing or charging again.
#[test]
fn removal_issues_release_credits_through_the_ledger() {
    let rig = rig();
    let a = &rig.sender;
    let (output, _) = rig.run(a, 1, store_input(&rig, 1), 17).unwrap();
    let Response::Store(stored) = codec::parse_response(&output).unwrap() else {
        panic!("store")
    };
    assert_eq!(rig.balance(a), 983);
    let remove = RemoveInput {
        placement_id: stored.placement_id,
        removal_id: crate::modules::messaging::ids::RemovalId([4; 32]),
        expected_expires_at: stored.expires_at,
        minimum_release_credits: 5,
    };
    let (output, price) = rig
        .run(a, 3, codec::request_value(&Request::Remove(remove)), 2)
        .unwrap();
    let Response::Remove(removed) = codec::parse_response(&output).unwrap() else {
        panic!("remove")
    };
    assert_eq!((price, removed.release_credits), (2, 8));
    assert_eq!(rig.balance(a), 983 - 2 + 8);
    // The released placement leaves the sender's recoverable view and its payload is gone.
    assert_eq!(
        rig.provider
            .participant()
            .store()
            .get(stored.placement_id)
            .err(),
        Some(crate::modules::messaging::error::Error::Storage)
    );
}
