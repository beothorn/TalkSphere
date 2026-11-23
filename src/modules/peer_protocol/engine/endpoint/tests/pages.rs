//! The real Page family over real Peer Protocol endpoints with volatile adapters: two independently owned module stacks (the publisher and a visitor)
//! exchange gets through quote, consent, reservation and commitment at the fixed price.
use super::*;
use crate::modules::{
    board::{self, definition::Definition, offering::Offering},
    hosted_publications::journal::Journal,
    identity::{api::IdentityId, text},
    messaging::{
        ports::Signer,
        test_support::{SequenceEntropy, TestIdentity},
    },
    pages::{
        media::MediaType,
        path::CanonicalPath,
        service::{
            client::Client,
            codec::{self, GetRequest},
            provider::PageProvider,
            standard::Catalogue,
            store::MemoryPages,
            terms::Terms,
            wire::Remote,
        },
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

const BUDGET: Budget = Budget {
    depth: 8,
    nodes: 4096,
    bytes: 65536,
};

// One world's Page family: the requester client and the provider adapter over that world's own resource map.
struct Family {
    client: Client,
    service: PageProvider,
    pages: Arc<MemoryPages>,
}

// The pinned offering published by `issuer`.
fn offering(issuer: IdentityId) -> Offering {
    let mut o: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../pages/service/fixtures/offering.json"
    ))
    .unwrap();
    o["creditIssuer"] = text::format(issuer).into();
    let raw = serde_json::json!({"version":1,"providedServices":[o],"wantedServices":[]});
    board::offering::Board::parse(raw.to_string().as_bytes(), Default::default())
        .unwrap()
        .offerings
        .remove("page")
        .unwrap()
}

// A family for the identity with `seed`, serving a page whose root is `abc`.
fn family(seed: u8) -> Arc<Family> {
    let identity = TestIdentity::new(seed);
    let pages = Arc::new(MemoryPages::default());
    pages.publish(
        CanonicalPath::root(),
        MediaType::parse("text/html").unwrap(),
        b"abc".to_vec(),
    );
    pages.publish(
        CanonicalPath::parse(b"/style.css").unwrap(),
        MediaType::parse("text/css").unwrap(),
        b"h1{}".to_vec(),
    );
    let catalogue = Catalogue::new(Default::default()).unwrap();
    let definition = catalogue
        .definition(&offering(identity.identity()))
        .unwrap();
    Arc::new(Family {
        client: Client::new(definition, &catalogue, BUDGET).unwrap(),
        service: PageProvider::new(
            identity.identity(),
            pages.clone(),
            Journal::volatile(64),
            Arc::new(SequenceEntropy::new(u64::from(seed))),
            BUDGET,
            65536,
        )
        .unwrap(),
        pages,
    })
}

impl WorldFamily for Family {
    // Each world publishes the pinned offering with itself as credit issuer.
    fn offering(&self, issuer: IdentityId) -> Offering {
        offering(issuer)
    }
    // The real pinned catalogue recognizes the Page contract.
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

// Build the two stacks, fund the visitor at the publisher and connect the visitor to the provider's metadata.
fn setup() -> (World<Family>, Ep, Client_, ()) {
    let (p, a, c) = (
        TestIdentity::new(2),
        TestIdentity::new(1),
        TestIdentity::new(3),
    );
    let provider_world = World::with_identities(p.identity(), a.identity(), false, family(2));
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
        let world = World::with_identities(identity.identity(), p.identity(), true, family(seed));
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
        let terms = Terms::parse(&o).unwrap();
        Client_ {
            endpoint,
            remote: Remote::new(terms, selection.clone(), cache.clone()),
            selection,
            cache,
            world,
        }
    };
    let visitor = build(TestIdentity::new(3), 3);
    (provider_world, provider, visitor, ())
}

// A get of `path` bounded by `maximum` bytes.
fn get(path: &str, maximum: u64) -> GetRequest {
    GetRequest {
        path: CanonicalPath::parse(path.as_bytes()).unwrap(),
        maximum_bytes: maximum,
    }
}

// Every get completes over real endpoints at the fixed price, charged once: the root, then an asset the root refers to, each with its own explicit plan
// and charge; a missing resource and a byte bound below the content fail without any charge.
#[test]
fn every_get_completes_over_real_endpoints_at_the_fixed_price() {
    let (_world, publisher, visitor, ()) = setup();
    let (plan, price) = visitor.remote.get(&get("/", 3)).unwrap();
    assert_eq!(price, 4);
    let (output, charged) = run(&visitor, &publisher, 5, plan).unwrap();
    let got = codec::parse_output(&output).unwrap();
    assert_eq!(
        (charged, got.content.as_slice(), got.media_type.as_str()),
        (4, &b"abc"[..], "text/html")
    );
    assert_eq!(owned(&visitor), 100 - 4);
    // Referenced assets are never fetched automatically: only the root was paid so far.
    assert_eq!(owned(&visitor), 96);
    let (plan, _) = visitor.remote.get(&get("/style.css", 100)).unwrap();
    let (output, _) = run(&visitor, &publisher, 7, plan).unwrap();
    assert_eq!(codec::parse_output(&output).unwrap().content, b"h1{}");
    assert_eq!(owned(&visitor), 92);
    for (stream, path, maximum) in [(9, "/missing", 100), (11, "/", 2)] {
        let (plan, _) = visitor.remote.get(&get(path, maximum)).unwrap();
        assert!(run(&visitor, &publisher, stream, plan).is_err(), "{path}");
    }
    assert_eq!(owned(&visitor), 92);
}

// The spending ceiling is the visitor's own exact price: a quote above the price the visitor calculates (stale terms) is refused before anything is
// reserved, and a request outside the offering's limits is refused locally.
#[test]
fn a_quote_above_the_visitors_own_price_is_refused() {
    let (_world, publisher, visitor, ()) = setup();
    let mut cheap = *visitor.remote.terms();
    cheap.price = 1;
    let remote = Remote::new(cheap, visitor.selection.clone(), visitor.cache.clone());
    let (plan, price) = remote.get(&get("/", 3)).unwrap();
    assert_eq!(price, 1);
    assert!(run(&visitor, &publisher, 5, plan).is_err());
    assert_eq!(owned(&visitor), 100);
    assert!(visitor.remote.get(&get("/", 0)).is_err());
    assert!(visitor.remote.get(&get("/", 101)).is_err());
    let long = format!("/{}", "a/".repeat(600).trim_end_matches('/'));
    assert!(visitor.remote.get(&get(&long, 3)).is_err());
}

// The publisher's page can change locally between visits: a new get sees the new snapshot and is separately charged.
#[test]
fn a_new_get_after_a_local_change_sees_the_new_snapshot() {
    let (world, publisher, visitor, ()) = setup();
    let (plan, _) = visitor.remote.get(&get("/", 100)).unwrap();
    run(&visitor, &publisher, 5, plan).unwrap();
    world.family.pages.publish(
        CanonicalPath::root(),
        MediaType::parse("text/html").unwrap(),
        b"new!".to_vec(),
    );
    let (plan, _) = visitor.remote.get(&get("/", 100)).unwrap();
    let (output, _) = run(&visitor, &publisher, 7, plan).unwrap();
    assert_eq!(codec::parse_output(&output).unwrap().content, b"new!");
    assert_eq!(owned(&visitor), 100 - 8);
}
