//! The real Messaging family over real Peer Protocol endpoints with volatile adapters: three independently owned module stacks (the
//! provider, the sender and the recipient) exchange every operation through quote, consent, reservation and commitment.
use super::*;
use crate::modules::{
    board::{self, definition::Definition, offering::Offering},
    identity::{api::IdentityId, text},
    messaging::{
        codec::{self, Response},
        ids::{MessageId, PlacementId, RecoveryId, RemovalId},
        journal::Journal,
        operation::{DiscoverInput, RemoveInput, RenewInput, StatusInput, StoreInput},
        participant::Participant,
        ports::Signer,
        provider::Provider as Engine,
        receipt,
        recovery::DeferToServices,
        service::{
            client::{Client, Placements},
            provider::ServiceProvider,
        },
        signing,
        staging::Staging,
        standard::Catalogue,
        state::Encoding,
        store::{Limits as StoreLimits, Store},
        terms::Terms,
        test_support::{FixedClock, SequenceEntropy, TestIdentity},
        wire::Remote,
    },
    peer_protocol::test_support::world::{Family as WorldFamily, World, ledger_context},
    services::{
        handler::{
            Assessment, Blob, Check, Completion, Invocation as ServiceInvocation, Peer, Provider,
            Requester, Status,
        },
        input::Input,
        validation::Semantics,
        value::Budget,
    },
};
use std::{collections::BTreeMap, sync::Mutex};

const BUDGET: Budget = Budget {
    depth: 8,
    nodes: 4096,
    bytes: 65536,
};

// What the requesters know about their placements: the payload size they stored or discovered.
#[derive(Default)]
struct Known(Mutex<BTreeMap<PlacementId, u64>>);
impl Placements for Known {
    // Sizes are known only for placements a test recorded, like a real requester's catalog.
    fn payload_size(
        &self,
        _: services::contract::OperationKey,
        _: IdentityId,
        id: PlacementId,
    ) -> Result<u64, services::Error> {
        self.0
            .lock()
            .unwrap()
            .get(&id)
            .copied()
            .ok_or(services::Error::Denied)
    }
}

// One world's Messaging family: the requester client and the provider adapter over that world's own engine.
struct Family {
    client: Client,
    service: ServiceProvider,
}

// The pinned offering published by `issuer`.
fn offering(issuer: IdentityId) -> Offering {
    let mut o: serde_json::Value =
        serde_json::from_str(include_str!("../../../../messaging/fixtures/offering.json")).unwrap();
    o["creditIssuer"] = text::format(issuer).into();
    let raw = serde_json::json!({"version":1,"providedServices":[o],"wantedServices":[]});
    board::offering::Board::parse(raw.to_string().as_bytes(), Default::default())
        .unwrap()
        .offerings
        .remove("messaging-standard")
        .unwrap()
}

// A family for the identity with `seed`, over a fresh volatile engine at time 1000.
fn family(seed: u8, known: &Arc<Known>) -> Arc<Family> {
    let identity = Arc::new(TestIdentity::new(seed));
    let clock = Arc::new(FixedClock::at(1000));
    let participant = Arc::new(Participant::new(
        Arc::new(Store::volatile(StoreLimits {
            placements: 16,
            payload_bytes: 10_000,
        })),
        Journal::volatile(64),
        Arc::new(SequenceEntropy::new(u64::from(seed))),
        clock.clone(),
        identity.clone(),
    ));
    participant.recover(&DeferToServices).unwrap();
    let terms = Terms::parse(&offering(identity.identity()), identity.identity()).unwrap();
    let engine = Arc::new(Engine::new(
        terms,
        participant,
        clock,
        Staging::new(10_000, 8),
    ));
    let catalogue = Catalogue::new(Default::default()).unwrap();
    let definition = catalogue
        .definition(&offering(identity.identity()))
        .unwrap();
    Arc::new(Family {
        client: Client::new(definition, &catalogue, known.clone(), BUDGET).unwrap(),
        service: ServiceProvider::new(engine, BUDGET, 65536).unwrap(),
    })
}

impl WorldFamily for Family {
    // Each world publishes the pinned offering with itself as credit issuer.
    fn offering(&self, issuer: IdentityId) -> Offering {
        offering(issuer)
    }
    // The real pinned catalogue recognizes the Messaging contract.
    fn standards() -> Arc<dyn services::registry::Standards> {
        Arc::new(Catalogue::new(Default::default()).unwrap())
    }
}

impl Semantics for Family {
    fn validate(
        &self,
        descriptor: &serde_json::Value,
        value: &Value,
    ) -> Result<(), services::Error> {
        self.client.validate(descriptor, value)
    }
    fn nonfinite(&self) -> bool {
        self.client.nonfinite()
    }
}
impl Requester for Family {
    fn definition(&self) -> &Definition {
        self.client.definition()
    }
    fn terms(&self, offering: &Offering) -> Result<(), services::Error> {
        self.client.terms(offering)
    }
    fn cached_retry(&self, input: &Value) -> Result<Option<services::replay::Id>, services::Error> {
        self.client.cached_retry(input)
    }
    fn pricing_for(
        &self,
        o: &Offering,
        m: &Value,
        f: &Value,
    ) -> Result<services::pricing::Price, services::Error> {
        self.client.pricing_for(o, m, f)
    }
    fn pricing_with_peer(
        &self,
        peer: Peer,
        o: &Offering,
        m: &Value,
        f: &Value,
    ) -> Result<services::pricing::Price, services::Error> {
        self.client.pricing_with_peer(peer, o, m, f)
    }
    fn assessment_metadata(&self, input: &Value, budget: Budget) -> Result<Value, services::Error> {
        self.client.assessment_metadata(input, budget)
    }
    fn assessment_metadata_with_peer(
        &self,
        peer: Peer,
        o: &Offering,
        input: &Value,
        budget: Budget,
    ) -> Result<Value, services::Error> {
        self.client
            .assessment_metadata_with_peer(peer, o, input, budget)
    }
    fn validate_assessment(
        &self,
        i: &Value,
        m: &Value,
        f: &Value,
        b: Budget,
    ) -> Result<(), services::Error> {
        self.client.validate_assessment(i, m, f, b)
    }
    fn validate_assessment_with_peer(
        &self,
        peer: Peer,
        o: &Offering,
        i: &Value,
        m: &Value,
        f: &Value,
        b: Budget,
    ) -> Result<(), services::Error> {
        self.client
            .validate_assessment_with_peer(peer, o, i, m, f, b)
    }
    fn validate_input(
        &self,
        metadata: &Value,
        input: &Value,
        budget: Budget,
    ) -> Result<(), services::Error> {
        self.client.validate_input(metadata, input, budget)
    }
    fn validate_result(
        &self,
        invocation: &ServiceInvocation,
        input: &Value,
        output: &Value,
    ) -> Result<(), services::Error> {
        self.client.validate_result(invocation, input, output)
    }
    fn checks(&self) -> Option<&dyn services::checks::Rules> {
        self.client.checks()
    }
    fn verify(
        &self,
        input: &Value,
        terms: &Blob,
        result: &Value,
        expectations: Option<&Value>,
    ) -> Result<Check, services::Error> {
        self.client.verify(input, terms, result, expectations)
    }
}
impl Provider for Family {
    fn allow_cached_retry(
        &self,
        original: &ServiceInvocation,
        peer: Peer,
        input: &Value,
    ) -> Result<bool, services::Error> {
        self.service.allow_cached_retry(original, peer, input)
    }
    fn assess(
        &self,
        peer: Peer,
        o: &Offering,
        input: &Value,
    ) -> Result<Assessment, services::Error> {
        self.service.assess(peer, o, input)
    }
    fn assess_metadata(
        &self,
        peer: Peer,
        o: &Offering,
        input: &Input,
        budget: Budget,
    ) -> Result<Assessment, services::Error> {
        self.service.assess_metadata(peer, o, input, budget)
    }
    fn execute(
        &self,
        invocation: &ServiceInvocation,
        input: &Value,
    ) -> Result<Completion, services::Error> {
        self.service.execute(invocation, input)
    }
    fn completion(
        &self,
        key: services::contract::OperationKey,
    ) -> Result<Completion, services::Error> {
        self.service.completion(key)
    }
    fn prepare(
        &self,
        key: services::contract::OperationKey,
        effect: &Blob,
    ) -> Result<Blob, services::Error> {
        self.service.prepare(key, effect)
    }
    fn validate_local_result(
        &self,
        invocation: &ServiceInvocation,
        input: &Value,
        result: &Completion,
    ) -> Result<bool, services::Error> {
        self.service
            .validate_local_result(invocation, input, result)
    }
    fn requires_finalization(&self, invocation: &ServiceInvocation, input: &Value) -> bool {
        self.service.requires_finalization(invocation, input)
    }
    fn validate_template(
        &self,
        invocation: &ServiceInvocation,
        input: &Value,
        template: &Completion,
    ) -> Result<(), services::Error> {
        self.service.validate_template(invocation, input, template)
    }
    fn finalize(
        &self,
        key: services::contract::OperationKey,
        token: &Blob,
        template: &Completion,
    ) -> Result<Completion, services::Error> {
        self.service.finalize(key, token, template)
    }
    fn commit_finalized(
        &self,
        key: services::contract::OperationKey,
        token: &Blob,
        template: &Completion,
        finalized: &Completion,
    ) -> Result<(), services::Error> {
        self.service
            .commit_finalized(key, token, template, finalized)
    }
    fn commit(
        &self,
        key: services::contract::OperationKey,
        token: &Blob,
    ) -> Result<(), services::Error> {
        self.service.commit(key, token)
    }
    fn abort(
        &self,
        key: services::contract::OperationKey,
        token: Option<&Blob>,
    ) -> Result<(), services::Error> {
        self.service.abort(key, token)
    }
    fn status(&self, key: services::contract::OperationKey) -> Result<Status, services::Error> {
        self.service.status(key)
    }
    fn evidence(
        &self,
        invocation: &services::checks::Invocation,
        input: &Value,
    ) -> Result<Value, services::Error> {
        self.service.evidence(invocation, input)
    }
}

type Ep = Endpoint<services::persistence::Memory, ledger::persistence::Memory>;

// One requester stack and the direct-metadata cache it built from the provider.
struct Client_ {
    endpoint: Ep,
    remote: Remote,
    selection: board::selection::Selection,
    cache: Arc<cache::PeerCache>,
    identity: TestIdentity,
    world: World<Family>,
}

// Read the committed output of an original requester operation.
fn output(client: &Client_, key: services::contract::OperationKey) -> (Value, u64) {
    let observe = client
        .world
        .services
        .grant(
            client.world.local,
            services::authority::Action::Observe,
            16_777_216,
        )
        .unwrap();
    let (_, completion) = client
        .world
        .runtime
        .completed(&peer::test_support::world::service_context(&observe), key)
        .unwrap();
    (completion.output, completion.price)
}

// Run one planned operation and return its committed output and price.
fn run(
    client: &Client_,
    provider: &Ep,
    stream: u32,
    plan: requests::Plan,
) -> Result<(Value, u64), Error> {
    match call(&client.endpoint, provider, stream, plan).result? {
        requests::Reply::Work(key) => Ok(output(client, key)),
        _ => Err(Error::Invalid),
    }
}

// The remote balance a requester owns from the provider's issuance.
fn owned(client: &Client_) -> u64 {
    let read = client
        .world
        .ledger_authority
        .grant(
            client.world.local,
            ledger::authority::Action::Read,
            16_777_216,
        )
        .unwrap();
    client
        .world
        .ledger
        .view(&ledger_context(&read), client.world.requester)
        .unwrap()
        .owned
}

// Build the three stacks, fund the recipient at the provider and connect both requesters to the provider's metadata.
fn setup() -> (World<Family>, Ep, Client_, Client_, Arc<Known>) {
    let known = Arc::new(Known::default());
    let (p, a, c) = (
        TestIdentity::new(2),
        TestIdentity::new(1),
        TestIdentity::new(3),
    );
    let provider_world =
        World::with_identities(p.identity(), a.identity(), false, family(2, &known));
    let admin = provider_world
        .ledger_authority
        .grant(
            p.identity(),
            ledger::authority::Action::Administer,
            16_777_216,
        )
        .unwrap();
    provider_world
        .ledger
        .grant(&ledger_context(&admin), c.identity(), 100)
        .unwrap();
    provider_world
        .ledger
        .set_trust(
            &ledger_context(&admin),
            c.identity(),
            ledger::accounting::Trust::new(1).unwrap(),
        )
        .unwrap();
    let provider = endpoint_world(&provider_world);
    let build = |identity: TestIdentity, seed: u8| {
        let world = World::with_identities(
            identity.identity(),
            p.identity(),
            true,
            family(seed, &known),
        );
        let endpoint = endpoint_world(&world);
        let cache =
            Arc::new(cache::PeerCache::new(world.registry.clone(), Default::default()).unwrap());
        call(
            &endpoint,
            &provider,
            1,
            requests::Plan::Board(cache.clone()),
        )
        .result
        .unwrap();
        let o = offering(p.identity());
        call(
            &endpoint,
            &provider,
            3,
            requests::Plan::Definition {
                cache: cache.clone(),
                offering: o.id.clone(),
            },
        )
        .result
        .unwrap();
        let selection = cache
            .selection(p.identity(), &o.id, &o.service, o.version)
            .unwrap();
        let terms = Terms::parse(&o, p.identity()).unwrap();
        Client_ {
            endpoint,
            remote: Remote::new(terms, selection.clone(), cache.clone()),
            selection,
            cache,
            identity,
            world,
        }
    };
    let sender = build(TestIdentity::new(1), 1);
    let recipient = build(TestIdentity::new(3), 3);
    (provider_world, provider, sender, recipient, known)
}

// Every Messaging operation completes over real endpoints at its exact price, charged once, and the exact retry of a recovery over a new
// stream is answered from the original output without a new charge.
#[test]
fn every_operation_completes_over_real_endpoints_at_its_exact_price() {
    let (_world, provider, a, c, known) = setup();
    let p = TestIdentity::new(2).identity();
    let message = signing::sign_message(
        &a.identity,
        c.identity.identity(),
        MessageId([1; 32]),
        1000,
        Encoding::Opaque,
        b"abc".to_vec(),
    )
    .unwrap();
    let store = StoreInput {
        message,
        retention_seconds: 10,
    };
    let (plan, price) = a.remote.store(&store).unwrap();
    assert_eq!(price, 17);
    let (output, charged) = run(&a, &provider, 5, plan).unwrap();
    assert_eq!(charged, 17);
    let Response::Store(stored) = codec::parse_response(&output).unwrap() else {
        panic!("store")
    };
    assert_eq!((stored.accepted_at, stored.expires_at), (1000, 1010));
    known.0.lock().unwrap().insert(stored.placement_id, 3);
    assert_eq!(owned(&a), 100 - 17);
    // The recipient discovers it and recovers it with its signed request.
    let (plan, price) = c.remote.discover(DiscoverInput {
        cursor: None,
        limit: 5,
    });
    let (output, _) = run(&c, &provider, 5, plan).unwrap();
    let Response::Discover(page) = codec::parse_response(&output).unwrap() else {
        panic!("discover")
    };
    assert_eq!((price, page.messages.len()), (1, 1));
    let request = receipt::sign_request(
        &c.identity,
        p,
        stored.placement_id,
        stored.message_id,
        store.message.metadata.payload_hash,
        RecoveryId([6; 32]),
    )
    .unwrap();
    let (plan, price) = c.remote.recover(request, 3).unwrap();
    let (first, _) = run(&c, &provider, 7, plan).unwrap();
    let Response::Recover(recovered) = codec::parse_response(&first).unwrap() else {
        panic!("recover")
    };
    assert_eq!(
        (price, recovered.message.payload.as_slice()),
        (2, &b"abc"[..])
    );
    assert_eq!(owned(&c), 100 - 1 - 2);
    // Status, check, renewal and removal by the sender.
    let (plan, _) = a.remote.status(StatusInput {
        placement_id: stored.placement_id,
    });
    let (output, _) = run(&a, &provider, 7, plan).unwrap();
    let Response::Status(status) = codec::parse_response(&output).unwrap() else {
        panic!("status")
    };
    assert_eq!(status.receipt, Some(recovered.receipt));
    let renew = RenewInput {
        placement_id: stored.placement_id,
        expected_expires_at: stored.expires_at,
        additional_seconds: 5,
    };
    let (plan, price) = a.remote.renew(renew, 3).unwrap();
    assert_eq!(price, 8);
    let (output, _) = run(&a, &provider, 9, plan).unwrap();
    let Response::Renew(renewed) = codec::parse_response(&output).unwrap() else {
        panic!("renew")
    };
    assert_eq!(renewed.expires_at, 1015);
    let before = owned(&a);
    let remove = RemoveInput {
        placement_id: stored.placement_id,
        removal_id: RemovalId([4; 32]),
        expected_expires_at: renewed.expires_at,
        minimum_release_credits: 0,
    };
    let (plan, price) = a.remote.remove(remove);
    // The placement was already recovered, so a removal is refused without any charge.
    assert!(run(&a, &provider, 11, plan).is_err());
    assert_eq!((price, owned(&a)), (2, before));
    // An exact retry of the recorded recovery over a new stream returns the original output and costs nothing.
    let balance = owned(&c);
    let Ok(requests::Reply::Work(key)) =
        call(&c.endpoint, &provider, 9, Remote::retry(first_key(&c))).result
    else {
        panic!("retry")
    };
    assert_eq!(output_of(&c, key), first);
    assert_eq!(owned(&c), balance);
    let _ = (&known, store);
}

// The key of the recipient's recovery, the only operation it ran at stream 7.
fn first_key(client: &Client_) -> services::contract::OperationKey {
    services::contract::OperationKey {
        requester: client.identity.identity(),
        connection: [19; 32],
        stream: 7,
    }
}

// The output retained for an original operation key.
fn output_of(client: &Client_, key: services::contract::OperationKey) -> Value {
    output(client, key).0
}

// A paid check over real endpoints charges its exact price once and the sender's private expectation turns the one provider byte into
// pass or fail; a wrong expectation is a paid failure and a missing one is inconclusive, never a pass.
#[test]
fn a_paid_check_is_judged_with_the_senders_private_expectation() {
    use crate::modules::messaging::operation::CheckInput;
    let (_world, provider, a, c, known) = setup();
    let message = signing::sign_message(
        &a.identity,
        c.identity.identity(),
        MessageId([1; 32]),
        1000,
        Encoding::Opaque,
        b"abc".to_vec(),
    )
    .unwrap();
    let (plan, _) = a
        .remote
        .store(&StoreInput {
            message,
            retention_seconds: 10,
        })
        .unwrap();
    let (output, _) = run(&a, &provider, 5, plan).unwrap();
    let Response::Store(stored) = codec::parse_response(&output).unwrap() else {
        panic!("store")
    };
    known.0.lock().unwrap().insert(stored.placement_id, 3);
    let before = owned(&a);
    let (plan, price) = a.remote.check(&CheckInput {
        placement_id: stored.placement_id,
        offset: 1,
    });
    assert_eq!(price, 3);
    let reply = call(&a.endpoint, &provider, 7, plan);
    let Ok(requests::Reply::Check(key)) = reply.result else {
        panic!("check completed")
    };
    assert_eq!(owned(&a), before - 3);
    let observe = a
        .world
        .services
        .grant(
            a.world.local,
            services::authority::Action::Observe,
            16_777_216,
        )
        .unwrap();
    let context = peer::test_support::world::service_context(&observe);
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
    assert_eq!(
        a.world
            .runtime
            .check_outcome(&context, key, Some(&expectation(b'b'))),
        Ok(Check::Pass)
    );
    assert_eq!(
        a.world
            .runtime
            .check_outcome(&context, key, Some(&expectation(b'x'))),
        Ok(Check::Fail)
    );
    assert_eq!(
        a.world.runtime.check_outcome(&context, key, None),
        Ok(Check::Inconclusive)
    );
}

// The spending ceiling is the requester's own exact price: when it calculates less than the provider quotes (stale terms), the quote is
// refused before anything is reserved, nothing is charged and nothing is stored.
#[test]
fn a_quote_above_the_requesters_own_price_is_refused() {
    let (world, provider, a, c, _) = setup();
    let mut cheap = *a.remote.terms();
    cheap.payload_usage = crate::modules::messaging::pricing::Rate {
        numerator: 1,
        denominator: 100,
    };
    cheap.payload_storage = crate::modules::messaging::pricing::Rate {
        numerator: 1,
        denominator: 100,
    };
    let remote = Remote::new(cheap, a.selection.clone(), a.cache.clone());
    let message = signing::sign_message(
        &a.identity,
        c.identity.identity(),
        MessageId([1; 32]),
        1000,
        Encoding::Opaque,
        b"abc".to_vec(),
    )
    .unwrap();
    let (plan, price) = remote
        .store(&StoreInput {
            message,
            retention_seconds: 10,
        })
        .unwrap();
    assert_eq!(price, 1);
    assert!(run(&a, &provider, 5, plan).is_err());
    assert_eq!(owned(&a), 100);
    let _ = world;
}
