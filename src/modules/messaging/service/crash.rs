//! SV-09 with Messaging effects: every coordination boundary reopens independent Services, Ledger and Messaging files.
//!
//! A node is built over three separately protected directories (the Services journal, the Ledger image and the Messaging placements
//! and participant journal). Each case drives an operation to a boundary, injects a lost or refused write, drops the whole node and
//! reopens it from the files alone. Recovery then resumes the coordinator's own decisions, and the tests assert that balances never
//! underflow, that placements and receipts exist exactly when the matching charge does, and that release credits apply once.
use super::{
    client::Client, client::Placements, provider::ServiceProvider, recovery, release::LedgerIssuer,
};
use crate::modules::{
    board, ledger,
    messaging::{
        codec::{self, Request, Response},
        ids::{MessageId, PlacementId, RecoveryId, RemovalId},
        journal::Journal,
        operation::{RecoverInput, RemoveInput, StoreInput},
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
        state::{Encoding, State},
        store::{Limits, Store},
        test_support::{FixedClock, SequenceEntropy, TestIdentity},
    },
    services::{
        self, Error,
        authority::{Action, Authority},
        contract::{Context, OperationKey},
        handler::{Blob, Peer},
        quote::{BoardCurrent, Issue, Quoter},
        runtime::{Config, Coordinator, Decision, Payment, Phase},
        value::{Budget, Value},
    },
};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

const BUDGET: Budget = Budget {
    depth: 8,
    nodes: 4096,
    bytes: 65536,
};
static NEXT: AtomicU64 = AtomicU64::new(0);

// A unique directory removed when the case ends, even after a failed assertion.
struct Dir(PathBuf);
impl Dir {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "talksphere-messaging-crash-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        )))
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
struct Dirs {
    messaging: Dir,
    services: Dir,
    ledger: Dir,
}

// The Services record store with an injected fault: 1 refuses the next write, 2 loses the acknowledgement of the next write.
struct ServiceDisk {
    store: services::linux::FileStore,
    fault: Arc<AtomicU8>,
}
impl services::persistence::Persistence for ServiceDisk {
    fn load(&mut self) -> Result<Option<services::persistence::Record>, Error> {
        self.store.load()
    }
    fn commit(&mut self, value: &services::persistence::Record) -> Result<(), Error> {
        let fault = self.fault.swap(0, Ordering::SeqCst);
        if fault == 1 {
            return Err(Error::Storage);
        }
        self.store.commit(value)?;
        if fault == 2 {
            Err(Error::Unknown)
        } else {
            Ok(())
        }
    }
}
// The Ledger image store; fault 2 loses the acknowledgement of the next durable write.
struct LedgerDisk {
    store: ledger::linux::FileStore,
    fault: Arc<AtomicU8>,
}
impl ledger::persistence::Persistence for LedgerDisk {
    fn load(&mut self) -> Result<Option<ledger::persistence::Record>, ledger::Error> {
        self.store.load()
    }
    fn commit(&mut self, value: &ledger::persistence::Record) -> Result<(), ledger::Error> {
        self.store.commit(value)?;
        if self.fault.swap(0, Ordering::SeqCst) == 2 {
            Err(ledger::Error::Unknown)
        } else {
            Ok(())
        }
    }
}

// The sizes the requesters know, shared with the tests.
#[derive(Default)]
struct Known(Mutex<BTreeMap<PlacementId, u64>>);
impl Placements for Known {
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

struct Node {
    clock: Arc<FixedClock>,
    engine: Arc<Provider>,
    service: Arc<ServiceProvider>,
    runtime: Coordinator<ServiceDisk>,
    registry: Arc<services::registry::Registry>,
    known: Arc<Known>,
    account: Arc<ledger::state::Ledger<LedgerDisk>>,
    authority: Arc<Authority>,
    ledger_authority: Arc<ledger::authority::Authority>,
    board: board::state::Provider,
    snapshot: board::contract::Snapshot,
    select: board::authority::Grant,
    quoter: Quoter,
    services_fault: Arc<AtomicU8>,
    ledger_fault: Arc<AtomicU8>,
    local: crate::modules::identity::api::IdentityId,
    deadline: Instant,
}

fn context(grant: &services::authority::Grant, deadline: Instant) -> Context<'_> {
    Context {
        version: 1,
        grant,
        deadline,
        cancelled: false,
    }
}
fn ledger_context(
    grant: &ledger::authority::Grant,
    deadline: Instant,
) -> ledger::contract::Context<'_> {
    ledger::contract::Context {
        version: 1,
        grant,
        deadline,
        cancelled: false,
    }
}

impl Node {
    // Rebuild every owner from the three directories; `create` initializes them for the first run, and `now` sets the clock.
    fn open(dirs: &Dirs, create: bool, now: u64) -> Self {
        let deadline = Instant::now() + Duration::from_secs(120);
        let clock = Arc::new(FixedClock::at(now));
        let identity = Arc::new(TestIdentity::new(2));
        let local = identity.identity();
        let (sender, recipient) = (TestIdentity::new(1), TestIdentity::new(3));

        let ledger_authority = Arc::new(ledger::authority::Authority::new());
        let admin = ledger_authority
            .grant(local, ledger::authority::Action::Administer, 16_777_216)
            .unwrap();
        let read = ledger_authority
            .grant(local, ledger::authority::Action::Read, 16_777_216)
            .unwrap();
        let coordinate = ledger_authority
            .grant(local, ledger::authority::Action::Coordinate, 16_777_216)
            .unwrap();
        let issuer_grant = ledger_authority
            .grant(local, ledger::authority::Action::Coordinate, 16_777_216)
            .unwrap();
        let ledger_fault = Arc::new(AtomicU8::new(0));
        let disk = LedgerDisk {
            store: ledger::linux::FileStore::open(&dirs.ledger.0, Default::default(), create)
                .unwrap(),
            fault: ledger_fault.clone(),
        };
        let account = Arc::new(if create {
            let lc = ledger_context(&admin, deadline);
            let bank = ledger::state::Ledger::initialize(
                local,
                ledger_authority.clone(),
                disk,
                Default::default(),
                &lc,
                10_000,
                100,
            )
            .unwrap();
            for who in [&sender, &recipient] {
                bank.grant(&lc, who.identity(), 1000).unwrap();
                bank.set_trust(
                    &lc,
                    who.identity(),
                    ledger::accounting::Trust::new(1).unwrap(),
                )
                .unwrap();
            }
            bank
        } else {
            ledger::state::Ledger::open(
                local,
                ledger_authority.clone(),
                disk,
                Default::default(),
                &ledger_context(&read, deadline),
            )
            .unwrap()
        });

        let store = Arc::new(
            Store::open(
                &dirs.messaging.0.join("placements"),
                Limits {
                    placements: 16,
                    payload_bytes: 10_000,
                },
            )
            .unwrap(),
        );
        let participant = Arc::new(
            Participant::new(
                store,
                Journal::open(&dirs.messaging.0.join("journal"), 64).unwrap(),
                Arc::new(SequenceEntropy::new(if create { 1 } else { 500 })),
                clock.clone(),
                identity.clone(),
            )
            .with_release_issuer(Arc::new(LedgerIssuer::new(
                account.clone(),
                issuer_grant,
                Duration::from_secs(60),
            ))),
        );
        participant.recover(&DeferToServices).unwrap();
        let engine = Arc::new(
            Provider::new(terms(), participant, clock.clone(), Staging::new(10_000, 8))
                .with_release_policy(Arc::new(|payments: u64, minimum: u64| {
                    (payments / 2).max(minimum)
                })),
        );
        let catalogue = Arc::new(Catalogue::new(Default::default()).unwrap());
        let definition = catalogue.definition(&offering()).unwrap();
        let known = Arc::new(Known::default());
        let family =
            Arc::new(Client::new(definition.clone(), &catalogue, known.clone(), BUDGET).unwrap());
        let service = Arc::new(ServiceProvider::new(engine.clone(), BUDGET, 65536).unwrap());
        let authority = Arc::new(Authority::new());
        let registry = Arc::new(
            services::registry::Registry::new(
                local,
                authority.clone(),
                Default::default(),
                catalogue,
            )
            .unwrap(),
        );
        let register = authority
            .grant(local, Action::Register, 16_777_216)
            .unwrap();
        let recover = authority.grant(local, Action::Recover, 16_777_216).unwrap();
        registry
            .register(&context(&register, deadline), family, Some(service.clone()))
            .unwrap();
        let accounting = Arc::new(services::accounting::Ledger::new(
            account.clone(),
            coordinate,
            Default::default(),
        ));
        let services_fault = Arc::new(AtomicU8::new(0));
        let disk = ServiceDisk {
            store: services::linux::FileStore::open(&dirs.services.0, Default::default(), create)
                .unwrap(),
            fault: services_fault.clone(),
        };
        let runtime = Coordinator::open(
            Config {
                local,
                authority: authority.clone(),
                limits: Default::default(),
            },
            &context(if create { &register } else { &recover }, deadline),
            disk,
            accounting,
            registry.clone(),
            create,
        )
        .unwrap_or_else(|e| panic!("coordinator open (create={create}): {e:?}"));

        let board_auth = Arc::new(board::authority::Authority::new());
        let board = board::state::Provider::new(
            local,
            board_auth.clone(),
            Default::default(),
            registry.clone(),
        )
        .unwrap();
        let publish = board_auth
            .grant(local, board::authority::Action::Publish, 16_777_216)
            .unwrap();
        let select = board_auth
            .grant(local, board::authority::Action::Select, 16_777_216)
            .unwrap();
        let mut published = offering().source.clone();
        published["creditIssuer"] = crate::modules::identity::text::format(local).into();
        let raw =
            serde_json::json!({"version":1,"providedServices":[published],"wantedServices":[]});
        let definitions = BTreeMap::from([(offering().id, definition.document.bytes().to_vec())]);
        let prepared = board::prepared::Prepared::new(
            local,
            &serde_json::to_vec(&raw).unwrap(),
            &definitions,
            Default::default(),
            registry.as_ref(),
        )
        .unwrap();
        let bp = board::contract::Context {
            grant: &publish,
            deadline,
            cancelled: false,
        };
        let snapshot = board.publish(&bp, prepared).unwrap();
        let quoter = Quoter::new(local, authority.clone(), Default::default()).unwrap();
        Self {
            clock,
            engine,
            service,
            runtime,
            registry,
            known,
            account,
            authority,
            ledger_authority,
            board,
            snapshot,
            select,
            quoter,
            services_fault,
            ledger_fault,
            local,
            deadline,
        }
    }

    fn cc<'a>(&self, grant: &'a services::authority::Grant) -> Context<'a> {
        context(grant, self.deadline)
    }

    // Admit one operation by `remote` (quote, accept, admit) and return its key.
    fn admit(
        &self,
        remote: &TestIdentity,
        stream: u32,
        input: &Value,
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
        let coordinate = self
            .authority
            .grant(self.local, Action::Coordinate, 16_777_216)
            .unwrap();
        let invoke = self
            .authority
            .grant(self.local, Action::Invoke, 16_777_216)
            .unwrap();
        let (cc, ic) = (self.cc(&coordinate), self.cc(&invoke));
        let bs = board::contract::Context {
            grant: &self.select,
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
                    service: "cr2se.messaging",
                    version: 1,
                    description: &o.description,
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
            input,
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
        self.runtime.admit(&cc, accepted, Payment::default())
    }
}

// Where a case stops, and which write is lost or refused on the way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cut {
    // The reservation write is durable but its acknowledgement is lost.
    AdmitLost,
    // Admitted and nothing else.
    Admitted,
    // Executed: the effect is prepared in the Messaging journal.
    Staged,
    // The preparation write is durable but its acknowledgement is lost.
    PrepareLost,
    // Prepared by the coordinator.
    Prepared,
    // The commit decision is recorded.
    Decided,
    // The commit completes durably but its acknowledgement is lost.
    CommitLost,
    // The first Services write of the commit is refused.
    CommitRefused,
    // The Ledger write inside the commit (such as the release issuance) is durable but its acknowledgement is lost.
    LedgerLost,
    // The coordinator decides to abort and delivers it.
    Aborted,
}

impl Node {
    // Drive one operation by `remote` up to `cut`, injecting the cut's fault; returns the operation key.
    fn drive(
        &self,
        remote: &TestIdentity,
        stream: u32,
        input: &Value,
        maximum: u64,
        cut: Cut,
    ) -> OperationKey {
        let key = OperationKey {
            requester: remote.identity(),
            connection: [9; 32],
            stream,
        };
        let coordinate = self
            .authority
            .grant(self.local, Action::Coordinate, 16_777_216)
            .unwrap();
        let cc = self.cc(&coordinate);
        if cut == Cut::AdmitLost {
            self.ledger_fault.store(2, Ordering::SeqCst);
            assert_eq!(
                self.admit(remote, stream, input, maximum),
                Err(Error::Unknown)
            );
            return key;
        }
        assert_eq!(self.admit(remote, stream, input, maximum), Ok(key));
        if cut == Cut::Admitted {
            return key;
        }
        self.runtime.supply_input(&cc, key, input.clone()).unwrap();
        self.runtime.start(&cc, key).unwrap();
        self.runtime.stage(&cc, key).unwrap();
        if cut == Cut::Staged {
            return key;
        }
        if cut == Cut::PrepareLost {
            self.ledger_fault.store(2, Ordering::SeqCst);
            assert_eq!(self.runtime.prepare(&cc, key), Err(Error::Unknown));
            return key;
        }
        self.runtime.prepare(&cc, key).unwrap();
        if cut == Cut::Prepared {
            return key;
        }
        if cut == Cut::Aborted {
            self.runtime.decide(&cc, key, Decision::Abort).unwrap();
            self.runtime.abort(&cc, key).unwrap();
            return key;
        }
        self.runtime.decide(&cc, key, Decision::Commit).unwrap();
        match cut {
            Cut::CommitLost => {
                self.services_fault.store(2, Ordering::SeqCst);
                assert!(self.runtime.commit(&cc, key).is_err());
            }
            Cut::CommitRefused => {
                self.services_fault.store(1, Ordering::SeqCst);
                assert!(self.runtime.commit(&cc, key).is_err());
            }
            Cut::LedgerLost => {
                self.ledger_fault.store(2, Ordering::SeqCst);
                assert!(self.runtime.commit(&cc, key).is_err());
            }
            _ => {}
        }
        key
    }

    // Complete one operation without any fault.
    fn complete(
        &self,
        remote: &TestIdentity,
        stream: u32,
        input: &Value,
        maximum: u64,
    ) -> OperationKey {
        let key = self.admit(remote, stream, input, maximum).unwrap();
        let coordinate = self
            .authority
            .grant(self.local, Action::Coordinate, 16_777_216)
            .unwrap();
        let cc = self.cc(&coordinate);
        self.runtime.supply_input(&cc, key, input.clone()).unwrap();
        self.runtime.start(&cc, key).unwrap();
        self.runtime.stage(&cc, key).unwrap();
        self.runtime.prepare(&cc, key).unwrap();
        self.runtime.decide(&cc, key, Decision::Commit).unwrap();
        self.runtime.commit(&cc, key).unwrap();
        key
    }

    // The issued balance the provider records for `who`.
    fn balance(&self, who: &TestIdentity) -> u64 {
        let read = self
            .ledger_authority
            .grant(self.local, ledger::authority::Action::Read, 16_777_216)
            .unwrap();
        self.account
            .view(&ledger_context(&read, self.deadline), who.identity())
            .unwrap()
            .issued
    }

    // Resume every unfinished operation through Services, twice, as a restarting node would (the second pass must change nothing).
    fn recover(&self) {
        let coordinate = self
            .authority
            .grant(self.local, Action::Coordinate, 16_777_216)
            .unwrap();
        for _ in 0..2 {
            recovery::resume(&self.runtime, &self.cc(&coordinate), self.service.as_ref()).unwrap();
        }
    }

    // The coordinator's phase and its retained output for an operation.
    fn outcome(&self, key: OperationKey) -> (Option<Phase>, Option<Value>) {
        let observe = self
            .authority
            .grant(self.local, Action::Observe, 16_777_216)
            .unwrap();
        let cc = self.cc(&observe);
        (
            self.runtime.status(&cc, key).unwrap(),
            self.runtime.result(&cc, key).ok().map(|r| r.output),
        )
    }
}

// The store of `abc` for ten seconds from `a` to `c`.
fn store_input(a: &TestIdentity, c: &TestIdentity, id: u8) -> Value {
    let message = crate::modules::messaging::signing::sign_message(
        a,
        c.identity(),
        MessageId([id; 32]),
        1000,
        Encoding::Opaque,
        b"abc".to_vec(),
    )
    .unwrap();
    codec::request_value(&Request::Store(StoreInput {
        message,
        retention_seconds: 10,
    }))
}

const ALL: [Cut; 10] = [
    Cut::AdmitLost,
    Cut::Admitted,
    Cut::Staged,
    Cut::PrepareLost,
    Cut::Prepared,
    Cut::Decided,
    Cut::CommitLost,
    Cut::CommitRefused,
    Cut::LedgerLost,
    Cut::Aborted,
];

// A store reopens at every boundary: before execution it is aborted with nothing charged or placed; from execution on it commits
// exactly once with a single charge and one placement, timed at the retained decision once that was recorded and at recovery otherwise.
#[test]
fn a_store_reopens_every_coordination_boundary() {
    for cut in ALL {
        let dirs = Dirs {
            messaging: Dir::new(),
            services: Dir::new(),
            ledger: Dir::new(),
        };
        let (a, c) = (TestIdentity::new(1), TestIdentity::new(3));
        let node = Node::open(&dirs, true, 1020);
        let key = node.drive(&a, 1, &store_input(&a, &c, 1), 17, cut);
        drop(node);
        let node = Node::open(&dirs, false, 1022);
        node.recover();
        let (phase, output) = node.outcome(key);
        let records = node.engine.participant().store().records();
        match cut {
            Cut::AdmitLost | Cut::Admitted | Cut::Aborted => {
                assert_eq!(phase, Some(Phase::Aborted), "{cut:?}");
                assert_eq!((node.balance(&a), records.len()), (1000, 0), "{cut:?}");
            }
            _ => {
                assert_eq!(phase, Some(Phase::Committed), "{cut:?}");
                assert_eq!((node.balance(&a), records.len()), (983, 1), "{cut:?}");
                let at = if matches!(
                    cut,
                    Cut::Decided | Cut::CommitLost | Cut::CommitRefused | Cut::LedgerLost
                ) {
                    1020
                } else {
                    1022
                };
                let Response::Store(stored) = codec::parse_response(&output.unwrap()).unwrap()
                else {
                    panic!("store")
                };
                assert_eq!(
                    (stored.accepted_at, stored.expires_at),
                    (at, at + 10),
                    "{cut:?}"
                );
                assert_eq!(
                    (records[0].state, records[0].accepted_at, records[0].id),
                    (State::Pending, at, stored.placement_id),
                    "{cut:?}"
                );
            }
        }
        assert_eq!(
            node.engine.participant().prepared().len(),
            0,
            "{cut:?}: nothing stays prepared"
        );
    }
}

// A recovery and a removal reopen at each commit-side boundary: the receipt and the removal exist exactly when the charge does, each
// charge applies once, and the release credits are issued once even when their acknowledgement was lost.
#[test]
fn recovery_and_removal_reopen_at_every_commit_boundary() {
    for removal in [false, true] {
        for cut in [
            Cut::Staged,
            Cut::PrepareLost,
            Cut::Prepared,
            Cut::Decided,
            Cut::CommitLost,
            Cut::CommitRefused,
            Cut::LedgerLost,
            Cut::Aborted,
        ] {
            let dirs = Dirs {
                messaging: Dir::new(),
                services: Dir::new(),
                ledger: Dir::new(),
            };
            let (a, c) = (TestIdentity::new(1), TestIdentity::new(3));
            let node = Node::open(&dirs, true, 1000);
            let stored = node.complete(&a, 1, &store_input(&a, &c, 1), 17);
            let Response::Store(stored) =
                codec::parse_response(&node.outcome(stored).1.unwrap()).unwrap()
            else {
                panic!("store")
            };
            node.known.0.lock().unwrap().insert(stored.placement_id, 3);
            let message = crate::modules::messaging::signing::sign_message(
                &a,
                c.identity(),
                MessageId([1; 32]),
                1000,
                Encoding::Opaque,
                b"abc".to_vec(),
            )
            .unwrap();
            let (remote, input, maximum) = if removal {
                let remove = RemoveInput {
                    placement_id: stored.placement_id,
                    removal_id: RemovalId([4; 32]),
                    expected_expires_at: stored.expires_at,
                    minimum_release_credits: 0,
                };
                (&a, codec::request_value(&Request::Remove(remove)), 2)
            } else {
                let request: RecoverInput = receipt::sign_request(
                    &c,
                    node.local,
                    stored.placement_id,
                    stored.message_id,
                    message.metadata.payload_hash,
                    RecoveryId([6; 32]),
                )
                .unwrap();
                (&c, codec::request_value(&Request::Recover(request)), 2)
            };
            node.clock.set(1004);
            let key = node.drive(remote, 3, &input, maximum, cut);
            drop(node);
            let node = Node::open(&dirs, false, 1006);
            node.recover();
            let (phase, output) = node.outcome(key);
            let record = node
                .engine
                .participant()
                .store()
                .record(stored.placement_id)
                .unwrap();
            let label = format!("removal={removal} {cut:?}");
            let committed = !matches!(
                cut,
                Cut::Staged | Cut::PrepareLost | Cut::Prepared | Cut::Aborted
            );
            let _ = committed;
            if cut == Cut::Aborted {
                assert_eq!(phase, Some(Phase::Aborted), "{label}");
                assert_eq!(
                    (
                        record.state,
                        record.receipt.is_none(),
                        record.removal.is_none()
                    ),
                    (State::Pending, true, true),
                    "{label}"
                );
                assert_eq!((node.balance(&a), node.balance(&c)), (983, 1000), "{label}");
                continue;
            }
            assert_eq!(phase, Some(Phase::Committed), "{label}");
            let at = if matches!(
                cut,
                Cut::Decided | Cut::CommitLost | Cut::CommitRefused | Cut::LedgerLost
            ) {
                1004
            } else {
                1006
            };
            if removal {
                let Response::Remove(done) = codec::parse_response(&output.unwrap()).unwrap()
                else {
                    panic!("remove")
                };
                assert_eq!(
                    (record.state, done.release_credits),
                    (State::Removed, 8),
                    "{label}"
                );
                assert_eq!(
                    node.balance(&a),
                    983 - 2 + 8,
                    "{label}: credits issued exactly once"
                );
                assert_eq!(node.balance(&c), 1000, "{label}");
            } else {
                let Response::Recover(done) = codec::parse_response(&output.unwrap()).unwrap()
                else {
                    panic!("recover")
                };
                assert_eq!(
                    (record.state, record.receipt),
                    (State::Recovered, Some(done.receipt)),
                    "{label}"
                );
                assert_eq!(done.receipt.recovered_at, at, "{label}");
                assert_eq!(
                    (node.balance(&c), node.balance(&a)),
                    (998, 983),
                    "{label}: one charge"
                );
            }
        }
    }
}
