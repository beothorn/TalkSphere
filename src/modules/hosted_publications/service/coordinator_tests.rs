//! Hosted publications through the real Services coordinator, Board and Ledger: quote, accept, stage, prepare, decide and commit for
//! every operation with exact charges, plus the requester client's pricing and result checks against hostile values.
use super::{
    client::{Client, Placements},
    provider::ServiceProvider,
};
use crate::modules::{
    board,
    hosted_publications::{
        codec::{self, Request, Response},
        ids::{PlacementId, PublicationId, RemovalId},
        journal::Journal,
        metadata::Kind,
        operation::{GetRequest, ListRequest, RemoveRequest, RenewRequest, StoreRequest},
        participant::Participant,
        provider::{Provider, tests::public},
        recovery::DeferToServices,
        signing::tests::VectorPublisher,
        standard::Catalogue,
        store::{Limits, Store},
        terms::tests::{offering, terms},
    },
    identity::api::IdentityId,
    ledger,
    messaging::{
        ports::Signer,
        service::release::LedgerIssuer,
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

// What the requesters know about their placements: the content size they stored, like a real requester's catalog.
#[derive(Default)]
struct Known(Mutex<BTreeMap<PlacementId, u64>>);
impl Placements for Known {
    // The size is known only for placements the test recorded.
    fn content_size(&self, _: OperationKey, _: IdentityId, id: PlacementId) -> Result<u64, Error> {
        self.0
            .lock()
            .unwrap()
            .get(&id)
            .copied()
            .ok_or(Error::Denied)
    }
}

struct Grants {
    coordinate: Grant,
    invoke: Grant,
    observe: Grant,
}

// A complete rig: provider engine, Services registry and coordinator, Board, Ledger and identities.
struct Rig {
    clock: Arc<FixedClock>,
    known: Arc<Known>,
    runtime: Coordinator<services::persistence::Memory>,
    registry: Arc<services::registry::Registry>,
    authority: Arc<Authority>,
    board: board::state::Provider,
    snapshot: board::contract::Snapshot,
    board_select: board::authority::Grant,
    quoter: Quoter,
    bank: Arc<ledger::state::Ledger<ledger::persistence::Memory>>,
    ledger_auth: Arc<ledger::authority::Authority>,
    grants: Grants,
    local: IdentityId,
    publisher: VectorPublisher,
    reader: TestIdentity,
    deadline: Instant,
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

    fn board_context(&self) -> board::contract::Context<'_> {
        board::contract::Context {
            grant: &self.board_select,
            deadline: self.deadline,
            cancelled: false,
        }
    }

    // Quote, accept, admit, supply the input, execute and stage one operation by `remote`, leaving the decision to the caller.
    fn stage(
        &self,
        remote: IdentityId,
        stream: u32,
        input: Value,
        maximum: u64,
    ) -> Result<OperationKey, Error> {
        let key = OperationKey {
            requester: remote,
            connection: [9; 32],
            stream,
        };
        let peer = Peer {
            local: self.local,
            remote,
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
                    service: "cr2se.hosted-publication",
                    version: 1,
                    description: &offering.description,
                    other: remote,
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

    // Run one operation by `remote` through the whole coordinator and return its retained output and price.
    fn run(
        &self,
        remote: IdentityId,
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

    // The balance the host has recognized for `who`'s issuer account, to check exact charging.
    fn balance(&self, who: IdentityId) -> u64 {
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
                who,
            )
            .unwrap()
            .issued
    }
}

// Build the rig: the engine at time 1000, registered Services handlers, a published offering and funded accounts for the publisher and
// the reader.
fn rig() -> Rig {
    let deadline = Instant::now() + Duration::from_secs(120);
    let clock = Arc::new(FixedClock::at(1000));
    let host = Arc::new(TestIdentity::new(2));
    let local = host.identity();
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
    let (publisher, reader) = (VectorPublisher::new(), TestIdentity::new(3));
    for who in [publisher.identity(), reader.identity()] {
        bank.grant(&bank_context, who, 1000).unwrap();
        bank.set_trust(
            &bank_context,
            who,
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
    let provider = Provider::new(
        terms(),
        Arc::new(Store::volatile(Limits {
            placements: 16,
            content_bytes: 10_000,
        })),
        clock.clone(),
        Arc::new(SequenceEntropy::new(1)),
        Arc::new(crate::modules::messaging::ports::FixedLimits::default()),
    )
    // Half of the cumulative payments are offered as early-release credits, never below the publisher's minimum.
    .with_release_policy(Arc::new(|payments: u64, minimum: u64| {
        (payments / 2).max(minimum)
    }))
    .with_release_issuer(Arc::new(LedgerIssuer::new(
        bank.clone(),
        issuer_grant,
        Duration::from_secs(60),
    )));
    let participant = Arc::new(Participant::new(
        Arc::new(provider),
        Journal::volatile(64),
        Arc::new(SequenceEntropy::new(2)),
    ));
    participant.recover(&DeferToServices).unwrap();
    let known = Arc::new(Known::default());
    let catalogue = Arc::new(Catalogue::new(Default::default()).unwrap());
    let definition = catalogue.definition(&offering()).unwrap();
    let family =
        Arc::new(Client::new(definition.clone(), &catalogue, known.clone(), BUDGET).unwrap());
    let service = Arc::new(ServiceProvider::new(participant, BUDGET, 65536).unwrap());
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
        known,
        runtime,
        registry,
        authority,
        board,
        snapshot,
        board_select,
        quoter,
        bank,
        ledger_auth,
        grants,
        local,
        publisher,
        reader,
        deadline,
    }
}

// The input value of a store of `abc` as a page at `sequence` for ten seconds.
fn store_input(rig: &Rig, id: u8, sequence: u64) -> Value {
    let publication = public(&rig.publisher, id, Kind::Page, sequence, b"abc");
    codec::request_value(&Request::Store(StoreRequest {
        publication,
        retention_seconds: 10,
    }))
}

// A full lifecycle through real Services and Ledger: each operation is quoted at its exact price, charged once and returns the engine's
// result, with the store time taken at the coordinator's decision (HP-02, HP-06, HP-11, HP-12 through Services).
#[test]
fn every_operation_runs_through_services_with_exact_charges() {
    let rig = rig();
    let (p, r) = (rig.publisher.identity(), rig.reader.identity());
    // Store: 17 credits; acceptance and expiry are the decision time.
    rig.clock.set(1020);
    let (output, price) = rig.run(p, 1, store_input(&rig, 1, 1), 17).unwrap();
    assert_eq!(price, 17);
    let Response::Store(stored, 17) = codec::parse_response(&output).unwrap() else {
        panic!("store")
    };
    assert_eq!((stored.accepted_at, stored.expires_at), (1020, 1030));
    rig.known.0.lock().unwrap().insert(stored.placement_id, 3);
    assert_eq!(rig.balance(p), 983);
    // List by the reader: 1 credit; the page appears.
    let list = codec::request_value(&Request::List(ListRequest {
        publisher_id: p,
        kind: Kind::Page,
        before: None,
        limit: 2,
    }));
    let (output, price) = rig.run(r, 3, list, 1).unwrap();
    let Response::List(page, 1) = codec::parse_response(&output).unwrap() else {
        panic!("list")
    };
    assert_eq!(
        (price, page.entries.len(), page.entries[0].placement_id),
        (1, 1, stored.placement_id)
    );
    // Get by the reader: 3 bytes at 1/2 is 2 credits, and the result is the signed publication, verified by the client.
    let get = codec::request_value(&Request::Get(GetRequest {
        publisher_id: p,
        publication_id: PublicationId([1; 32]),
        maximum_bytes: 100,
    }));
    let (output, price) = rig.run(r, 5, get, 2).unwrap();
    let Response::Get(got, 2) = codec::parse_response(&output).unwrap() else {
        panic!("get")
    };
    assert_eq!(
        (price, got.publication.content.as_slice()),
        (2, &b"abc"[..])
    );
    assert_eq!(rig.balance(r), 1000 - 1 - 2);
    // Renew by 5 seconds: 3 bytes for 5 seconds at 1/2 is 7.5, charged 8.
    rig.clock.set(1025);
    let renew = RenewRequest {
        placement_id: stored.placement_id,
        expected_expires_at: stored.expires_at,
        additional_seconds: 5,
    };
    let (output, price) = rig
        .run(p, 7, codec::request_value(&Request::Renew(renew)), 8)
        .unwrap();
    let Response::Renew(renewed, 8) = codec::parse_response(&output).unwrap() else {
        panic!("renew")
    };
    assert_eq!((price, renewed.expires_at), (8, stored.expires_at + 5));
    // Remove: 2 credits, with half of the 25 credits paid released (12).
    let remove = RemoveRequest {
        placement_id: stored.placement_id,
        removal_id: RemovalId([4; 32]),
        expected_expires_at: renewed.expires_at,
        minimum_release_credits: 0,
    };
    let (output, price) = rig
        .run(p, 9, codec::request_value(&Request::Remove(remove)), 2)
        .unwrap();
    let Response::Remove(removed) = codec::parse_response(&output).unwrap() else {
        panic!("remove")
    };
    assert_eq!(
        (price, removed.removed, removed.release_credits),
        (2, true, 12)
    );
    assert_eq!(rig.balance(p), 1000 - 17 - 8 - 2 + 12);
    // Once removed the publication is gone: a get is concealed as unavailable and nothing is charged.
    let again = codec::request_value(&Request::Get(GetRequest {
        publisher_id: p,
        publication_id: PublicationId([1; 32]),
        maximum_bytes: 100,
    }));
    assert!(rig.run(r, 11, again, 2).is_err());
    assert_eq!(rig.balance(r), 997);
    let _ = (&rig.registry, &rig.authority);
}

// The client prices every quote exactly and refuses what a hostile provider might state: a wrong or missing factor, a get size above the
// requester's bound, a renewal size the requester does not know, a removal below the requested minimum and an unknown operation.
#[test]
fn the_client_prices_exactly_and_refuses_bad_factors() {
    use services::handler::Requester;
    let rig = rig();
    let (p, r) = (rig.publisher.identity(), rig.reader.identity());
    let catalogue = Catalogue::new(Default::default()).unwrap();
    let definition = catalogue.definition(&offering()).unwrap();
    let client = Client::new(definition, &catalogue, rig.known.clone(), BUDGET).unwrap();
    let peer = |who: IdentityId| Peer {
        local: rig.local,
        remote: who,
        key: OperationKey {
            requester: who,
            connection: [1; 32],
            stream: 1,
        },
    };
    let object = |fields: Vec<(&str, Value)>| {
        Value::Object(fields.into_iter().map(|(n, v)| (n.into(), v)).collect())
    };
    let price = |who, metadata: &Value, factors: &Value| {
        client.pricing_with_peer(peer(who), &offering(), metadata, factors)
    };
    let empty = object(vec![]);
    let store = codec::request_value(&Request::Store(StoreRequest {
        publication: public(&rig.publisher, 1, Kind::Page, 1, b"abc"),
        retention_seconds: 10,
    }));
    let projected = client.assessment_metadata(&store, BUDGET).unwrap();
    assert_eq!(
        price(p, &projected, &empty),
        Ok(services::pricing::Price::Fixed(17))
    );
    assert_eq!(
        price(p, &projected, &object(vec![("x", Value::Uint64(1))])).err(),
        Some(Error::Invalid)
    );
    let get = |max: u64| {
        codec::request_value(&Request::Get(GetRequest {
            publisher_id: p,
            publication_id: PublicationId([1; 32]),
            maximum_bytes: max,
        }))
    };
    let size = |n: u64| object(vec![("contentSize", Value::Uint64(n))]);
    assert_eq!(
        price(r, &get(100), &size(3)),
        Ok(services::pricing::Price::Fixed(2))
    );
    // A size above what the requester allowed, zero, above the offering, a missing or extra factor are all invalid.
    for bad in [
        size(4),
        size(0),
        size(101),
        empty.clone(),
        object(vec![
            ("contentSize", Value::Uint64(3)),
            ("y", Value::Uint64(1)),
        ]),
    ] {
        assert_eq!(
            price(r, &get(3), &bad).err(),
            Some(Error::Invalid),
            "{bad:?}"
        );
    }
    // A renewal size the requester does not know, or knows differently, is refused.
    let renew = codec::request_value(&Request::Renew(RenewRequest {
        placement_id: PlacementId([7; 32]),
        expected_expires_at: 1010,
        additional_seconds: 10,
    }));
    assert_eq!(price(p, &renew, &size(3)).err(), Some(Error::Denied));
    rig.known.0.lock().unwrap().insert(PlacementId([7; 32]), 3);
    assert_eq!(
        price(p, &renew, &size(3)),
        Ok(services::pricing::Price::Fixed(15))
    );
    assert_eq!(price(p, &renew, &size(4)).err(), Some(Error::Invalid));
    // A removal must offer at least the requested minimum.
    let remove = |minimum| {
        codec::request_value(&Request::Remove(RemoveRequest {
            placement_id: PlacementId([7; 32]),
            removal_id: RemovalId([1; 32]),
            expected_expires_at: 1010,
            minimum_release_credits: minimum,
        }))
    };
    let credits = |n: u64| object(vec![("releaseCredits", Value::Uint64(n))]);
    assert_eq!(
        price(p, &remove(5), &credits(5)),
        Ok(services::pricing::Price::Fixed(2))
    );
    assert_eq!(
        price(p, &remove(5), &credits(4)).err(),
        Some(Error::Invalid)
    );
    // The cached retry identifier is the removal identifier only.
    assert!(client.cached_retry(&remove(0)).unwrap().is_some());
    assert!(client.cached_retry(&get(3)).unwrap().is_none());
    assert_eq!(
        price(
            p,
            &object(vec![("operation", Value::String("check".into()))]),
            &empty
        )
        .err(),
        Some(Error::Invalid)
    );
}

// An invocation for result and check validation, with the given provider factors.
fn invocation(factors: Value) -> services::handler::Invocation {
    let offering = offering();
    let definition = Catalogue::new(Default::default())
        .unwrap()
        .definition(&offering)
        .unwrap();
    let accepted = Blob::new(b"accepted".to_vec(), 64).unwrap();
    let requester = IdentityId([5; 32]);
    services::handler::Invocation {
        key: OperationKey {
            requester,
            connection: [1; 32],
            stream: 1,
        },
        requester,
        provider: IdentityId([2; 32]),
        accepted: accepted.clone(),
        maximum: 100,
        publisher: IdentityId([2; 32]),
        issuer: offering.issuer,
        offering,
        definition,
        quote: accepted,
        factors,
        pricing: services::pricing::Price::Fixed(1),
    }
}

// HP-06/HP-08 requester side: a result must belong to its request. A store result for another publication or with a wrong expiry, a list
// that is too long, of another kind or out of order, a get whose publication fails the publisher's signature, names another publication,
// exceeds the bound or differs from the quoted size, and renewal and removal results that disagree are all refused, while the honest
// result of each passes.
#[test]
fn results_must_belong_to_their_request() {
    use crate::modules::hosted_publications::{
        ids::ContentHash,
        operation::{GetOutput, ListEntry, ListOutput, RemoveOutput, RenewOutput, StoreOutput},
    };
    use services::handler::Requester;
    let rig = rig();
    let p = rig.publisher.identity();
    let catalogue = Catalogue::new(Default::default()).unwrap();
    let definition = catalogue.definition(&offering()).unwrap();
    let client = Client::new(definition, &catalogue, rig.known.clone(), BUDGET).unwrap();
    let check = |request: Request, response: Response, factors: Value| {
        client.validate_result(
            &invocation(factors),
            &codec::request_value(&request),
            &codec::response_value(&response),
        )
    };
    let none = Value::Object(vec![]);
    let size = |n: u64| Value::Object(vec![("contentSize".into(), Value::Uint64(n))]);
    // Store.
    let publication = public(&rig.publisher, 1, Kind::Page, 1, b"abc");
    let store = Request::Store(StoreRequest {
        publication: publication.clone(),
        retention_seconds: 10,
    });
    let stored = |id: u8, expires: u64| {
        Response::Store(
            StoreOutput {
                placement_id: PlacementId([8; 32]),
                publication_id: PublicationId([id; 32]),
                accepted_at: 1000,
                expires_at: expires,
            },
            17,
        )
    };
    assert!(check(store.clone(), stored(1, 1010), none.clone()).is_ok());
    assert!(check(store.clone(), stored(2, 1010), none.clone()).is_err());
    assert!(check(store.clone(), stored(1, 1011), none.clone()).is_err());
    // List.
    let entry = |sequence: u64, id: u8, kind: Kind| ListEntry {
        placement_id: PlacementId([id; 32]),
        publication_id: PublicationId([id; 32]),
        kind,
        sequence,
        published_at: 1,
        visibility: crate::modules::hosted_publications::metadata::Visibility::Public,
        content_size: 3,
        content_hash: ContentHash([0; 32]),
        accepted_at: 1,
        expires_at: 2,
    };
    let list = Request::List(ListRequest {
        publisher_id: p,
        kind: Kind::Page,
        before: Some((9, PublicationId([9; 32]))),
        limit: 2,
    });
    let page = |entries: Vec<ListEntry>| {
        Response::List(
            ListOutput {
                publisher_id: p,
                entries,
                more: false,
            },
            1,
        )
    };
    assert!(
        check(
            list.clone(),
            page(vec![entry(8, 1, Kind::Page), entry(7, 2, Kind::Page)]),
            none.clone()
        )
        .is_ok()
    );
    assert!(check(list.clone(), page(vec![]), none.clone()).is_ok());
    for bad in [
        vec![
            entry(8, 1, Kind::Page),
            entry(7, 2, Kind::Page),
            entry(6, 3, Kind::Page),
        ],
        vec![entry(8, 1, Kind::FeedPost)],
        vec![entry(7, 2, Kind::Page), entry(8, 1, Kind::Page)],
        vec![entry(8, 1, Kind::Page), entry(8, 1, Kind::Page)],
        vec![entry(9, 9, Kind::Page)],
        vec![entry(10, 1, Kind::Page)],
    ] {
        assert!(check(list.clone(), page(bad), none.clone()).is_err());
    }
    // Get: the honest publication passes; a forged signature, another publication or publisher, a larger size than allowed and a size
    // different from the quote do not.
    let get = |max: u64| {
        Request::Get(GetRequest {
            publisher_id: p,
            publication_id: PublicationId([1; 32]),
            maximum_bytes: max,
        })
    };
    let got = |publication: crate::modules::hosted_publications::metadata::Publication| {
        Response::Get(GetOutput { publication }, 2)
    };
    assert!(check(get(100), got(publication.clone()), size(3)).is_ok());
    let mut forged = publication.clone();
    forged.content[0] ^= 1;
    assert!(check(get(100), got(forged), size(3)).is_err());
    let mut other_id = publication.clone();
    other_id.metadata.publication_id = PublicationId([2; 32]);
    assert!(check(get(100), got(other_id), size(3)).is_err());
    assert!(check(get(2), got(publication.clone()), size(3)).is_err());
    assert!(check(get(100), got(publication.clone()), size(4)).is_err());
    assert!(check(get(100), got(publication), none.clone()).is_err());
    // Renew and remove.
    let renew = Request::Renew(RenewRequest {
        placement_id: PlacementId([8; 32]),
        expected_expires_at: 1010,
        additional_seconds: 5,
    });
    let renewed = |previous: u64, expires: u64| {
        Response::Renew(
            RenewOutput {
                placement_id: PlacementId([8; 32]),
                previous_expires_at: previous,
                expires_at: expires,
            },
            8,
        )
    };
    assert!(check(renew.clone(), renewed(1010, 1015), size(3)).is_ok());
    assert!(check(renew.clone(), renewed(1011, 1016), size(3)).is_err());
    assert!(check(renew, renewed(1010, 1016), size(3)).is_err());
    let remove = Request::Remove(RemoveRequest {
        placement_id: PlacementId([8; 32]),
        removal_id: RemovalId([4; 32]),
        expected_expires_at: 1010,
        minimum_release_credits: 5,
    });
    let removed = |credits: u64, flag: bool| {
        Response::Remove(RemoveOutput {
            placement_id: PlacementId([8; 32]),
            removal_id: RemovalId([4; 32]),
            removed: flag,
            release_credits: credits,
            price: 2,
        })
    };
    assert!(check(remove.clone(), removed(5, true), none.clone()).is_ok());
    assert!(check(remove.clone(), removed(4, true), none.clone()).is_err());
    assert!(check(remove.clone(), removed(5, false), none.clone()).is_err());
    // A response of another operation never belongs to the request.
    assert!(check(remove, stored(1, 1010), none).is_err());
}

// HP-13 requester side: every challenge is separately paid; the offset must lie inside the content the requester knows; evidence must
// answer exactly the challenge; pass or fail needs timely matching evidence and the private expectation, otherwise inconclusive.
#[test]
fn byte_checks_are_paid_bounded_and_judged_privately() {
    use crate::modules::hosted_publications::operation::{CheckOutput, CheckRequest};
    use services::{
        checks::{Mode, Timing},
        handler::{Check, Requester},
    };
    let rig = rig();
    let catalogue = Catalogue::new(Default::default()).unwrap();
    let definition = catalogue.definition(&offering()).unwrap();
    let client = Client::new(definition.clone(), &catalogue, rig.known.clone(), BUDGET).unwrap();
    let rules = client.checks().unwrap();
    let placement = PlacementId([8; 32]);
    rig.known.0.lock().unwrap().insert(placement, 3);
    let challenge = |offset: u64| {
        codec::check_input_value(&CheckRequest {
            placement_id: placement,
            offset,
        })
    };
    assert!(matches!(
        rules.mode(&offering(), &challenge(1)),
        Ok(Mode::Paid)
    ));
    assert_eq!(rules.response_milliseconds(&offering()), Ok(5000));
    let check_invocation = |price: u64| services::checks::Invocation {
        key: OperationKey {
            requester: IdentityId([5; 32]),
            connection: [1; 32],
            stream: 1,
        },
        requester: IdentityId([5; 32]),
        provider: IdentityId([2; 32]),
        issuer: offering().issuer,
        offering: offering(),
        definition: definition.clone(),
        quote: Blob::new(b"quote".to_vec(), 16).unwrap(),
        price,
        response_milliseconds: 5000,
        deadline: Instant::now() + Duration::from_secs(5),
    };
    assert!(rules.challenge(&check_invocation(3), &challenge(2)).is_ok());
    // A challenge past the known content, with the wrong price, or for an unknown placement is refused.
    assert!(
        rules
            .challenge(&check_invocation(3), &challenge(3))
            .is_err()
    );
    assert!(
        rules
            .challenge(&check_invocation(2), &challenge(1))
            .is_err()
    );
    let unknown = codec::check_input_value(&CheckRequest {
        placement_id: PlacementId([9; 32]),
        offset: 0,
    });
    assert_eq!(
        rules.challenge(&check_invocation(3), &unknown).err(),
        Some(Error::Denied)
    );
    let evidence = |offset: u64, value: u8| {
        codec::check_output_value(&CheckOutput {
            placement_id: placement,
            offset,
            value,
        })
    };
    assert!(rules.evidence(&challenge(1), &evidence(1, b'x')).is_ok());
    assert!(rules.evidence(&challenge(1), &evidence(2, b'x')).is_err());
    let expectation = Value::Object(vec![
        ("placementId".into(), Value::Bytes(placement.0.to_vec())),
        ("offset".into(), Value::Uint64(1)),
        ("expectedByte".into(), Value::Bytes(vec![b'b'])),
    ]);
    let judge = |evidence: Option<&Value>, expectations: Option<&Value>, timing: Timing| {
        rules.outcome(&challenge(1), evidence, expectations, timing)
    };
    let within = Timing::Within(10);
    assert_eq!(
        judge(Some(&evidence(1, b'b')), Some(&expectation), within),
        Check::Pass
    );
    assert_eq!(
        judge(Some(&evidence(1, b'x')), Some(&expectation), within),
        Check::Fail
    );
    for timing in [Timing::Ambiguous, Timing::Missing] {
        assert_eq!(
            judge(Some(&evidence(1, b'x')), Some(&expectation), timing),
            Check::Inconclusive
        );
    }
    assert_eq!(judge(None, Some(&expectation), within), Check::Inconclusive);
    assert_eq!(
        judge(Some(&evidence(1, b'b')), None, within),
        Check::Inconclusive
    );
    assert_eq!(
        judge(Some(&evidence(2, b'b')), Some(&expectation), within),
        Check::Inconclusive
    );
}
