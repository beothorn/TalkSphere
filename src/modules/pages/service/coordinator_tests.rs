//! The Page service through the real Services coordinator, Board and Ledger (PG-02, PG-10, PG-11): every `get` is quoted at the fixed price, charged once
//! and returns exactly the staged snapshot; failed gets and unauthorized assets cost nothing.
use super::{
    client::Client,
    codec::{self, GetRequest},
    provider::PageProvider,
    standard::Catalogue,
    store::MemoryPages,
    terms::tests::offering,
};
use crate::modules::{
    board,
    hosted_publications::journal::Journal,
    identity::api::IdentityId,
    ledger,
    messaging::{
        ports::Signer,
        test_support::{SequenceEntropy, TestIdentity},
    },
    pages::{media::MediaType, path::CanonicalPath},
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
    sync::Arc,
    time::{Duration, Instant},
};

const BUDGET: Budget = Budget {
    depth: 8,
    nodes: 4096,
    bytes: 65536,
};

struct Rig {
    pages: Arc<MemoryPages>,
    runtime: Coordinator<services::persistence::Memory>,
    registry: Arc<services::registry::Registry>,
    authority: Arc<Authority>,
    board: board::state::Provider,
    snapshot: board::contract::Snapshot,
    board_select: board::authority::Grant,
    quoter: Quoter,
    bank: Arc<ledger::state::Ledger<ledger::persistence::Memory>>,
    ledger_auth: Arc<ledger::authority::Authority>,
    coordinate: Grant,
    invoke: Grant,
    observe: Grant,
    local: IdentityId,
    visitor: TestIdentity,
    deadline: Instant,
}

impl Rig {
    fn context<'a>(&self, grant: &'a Grant) -> Context<'a> {
        Context {
            version: 1,
            grant,
            deadline: self.deadline,
            cancelled: false,
        }
    }

    // Quote, accept, admit, supply the input, execute and stage one get by the visitor.
    fn stage(&self, stream: u32, input: Value, maximum: u64) -> Result<OperationKey, Error> {
        let remote = self.visitor.identity();
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
        let (cc, ic) = (self.context(&self.coordinate), self.context(&self.invoke));
        let bs = board::contract::Context {
            grant: &self.board_select,
            deadline: self.deadline,
            cancelled: false,
        };
        let o = offering();
        let selection = self
            .board
            .select(
                &bs,
                &self.snapshot,
                &board::selection::Request {
                    id: &o.id,
                    service: "cr2se.page",
                    version: 1,
                    description: &o.description,
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

    // Run one get through the whole coordinator and return its retained output and price.
    fn run(&self, stream: u32, input: Value, maximum: u64) -> Result<(Value, u64), Error> {
        let key = self.stage(stream, input, maximum)?;
        let (cc, oc) = (self.context(&self.coordinate), self.context(&self.observe));
        self.runtime.prepare(&cc, key)?;
        self.runtime.decide(&cc, key, Decision::Commit)?;
        self.runtime.commit(&cc, key)?;
        let result = self.runtime.result(&oc, key)?;
        Ok((result.output, result.price))
    }

    // The balance the publisher has recognized for the visitor's issuer account.
    fn balance(&self) -> u64 {
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
                self.visitor.identity(),
            )
            .unwrap()
            .issued
    }
}

fn rig() -> Rig {
    let deadline = Instant::now() + Duration::from_secs(120);
    let host = TestIdentity::new(2);
    let local = host.identity();
    let visitor = TestIdentity::new(3);
    let ledger_auth = Arc::new(ledger::authority::Authority::new());
    let admin = ledger_auth
        .grant(local, ledger::authority::Action::Administer, 1_000_000)
        .unwrap();
    let lc = ledger::contract::Context {
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
            &lc,
            10_000,
            100,
        )
        .unwrap(),
    );
    bank.grant(&lc, visitor.identity(), 1000).unwrap();
    bank.set_trust(
        &lc,
        visitor.identity(),
        ledger::accounting::Trust::new(1).unwrap(),
    )
    .unwrap();
    let coordinate_ledger = ledger_auth
        .grant(local, ledger::authority::Action::Coordinate, 1_000_000)
        .unwrap();
    let accounting = Arc::new(services::accounting::Ledger::new(
        bank.clone(),
        coordinate_ledger,
        Default::default(),
    ));
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
    let provider = Arc::new(
        PageProvider::new(
            local,
            pages.clone(),
            Journal::volatile(64),
            Arc::new(SequenceEntropy::new(1)),
            BUDGET,
            65536,
        )
        .unwrap(),
    );
    let catalogue = Arc::new(Catalogue::new(Default::default()).unwrap());
    let definition = catalogue.definition(&offering()).unwrap();
    let family = Arc::new(Client::new(definition.clone(), &catalogue, BUDGET).unwrap());
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
    registry.register(&rc, family, Some(provider)).unwrap();
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
    let coordinate = authority
        .grant(local, Action::Coordinate, 1_000_000)
        .unwrap();
    let invoke = authority.grant(local, Action::Invoke, 1_000_000).unwrap();
    let observe = authority.grant(local, Action::Observe, 1_000_000).unwrap();
    Rig {
        pages,
        runtime,
        registry,
        authority,
        board,
        snapshot,
        board_select,
        quoter,
        bank,
        ledger_auth,
        coordinate,
        invoke,
        observe,
        local,
        visitor,
        deadline,
    }
}

fn get(path: &str, maximum: u64) -> Value {
    codec::request_value(&GetRequest {
        path: CanonicalPath::parse(path.as_bytes()).unwrap(),
        maximum_bytes: maximum,
    })
}

// PG-02, PG-07, PG-11: each get is quoted at the fixed price 4 and charged once; the root authorizes only the root, an asset needs its own explicit
// invocation and charge, a failed get costs nothing, and a new get after a replacement sees the new bytes at a new charge.
#[test]
fn every_get_is_charged_once_at_the_fixed_price() {
    let rig = rig();
    let (output, price) = rig.run(1, get("/", 3), 4).unwrap();
    let got = codec::parse_output(&output).unwrap();
    assert_eq!(
        (price, got.content.as_slice(), got.price),
        (4, &b"abc"[..], 4)
    );
    assert_eq!(rig.balance(), 996);
    // The root references an asset, but only the root was authorized and paid: nothing else was invoked.
    assert_eq!(rig.balance(), 996);
    let (asset, price) = rig.run(2, get("/style.css", 100), 4).unwrap();
    assert_eq!(
        (
            price,
            codec::parse_output(&asset).unwrap().content.as_slice()
        ),
        (4, &b"h1{}"[..])
    );
    assert_eq!(rig.balance(), 992);
    // A missing resource and an oversized bound fail without any charge.
    assert!(rig.run(3, get("/missing", 100), 4).is_err());
    assert!(rig.run(4, get("/", 2), 4).is_err());
    assert_eq!(rig.balance(), 992);
    // After a local replacement a new get returns the new snapshot at a separate charge.
    rig.pages.publish(
        CanonicalPath::root(),
        MediaType::parse("text/html").unwrap(),
        b"new!".to_vec(),
    );
    let (fresh, price) = rig.run(5, get("/", 100), 4).unwrap();
    assert_eq!(
        (
            price,
            codec::parse_output(&fresh).unwrap().content.as_slice()
        ),
        (4, &b"new!"[..])
    );
    assert_eq!(rig.balance(), 988);
    let _ = (&rig.registry, &rig.authority);
}
