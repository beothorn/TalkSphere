//! Every SV-09 local coordination cut reopens independently protected Storage, Services and Ledger.
use super::*;
use crate::modules::storage::{bytes::tests::PathGuard, contract::State, recovery::Services};
use services::runtime::Phase;
use std::sync::atomic::{AtomicU8, Ordering};

struct ServiceDisk {
    store: services::linux::FileStore,
    fault: Arc<AtomicU8>,
}
impl services::persistence::Persistence for ServiceDisk {
    // Delegate complete authenticated owner recovery to the actual native store.
    fn load(&mut self) -> Result<Option<services::persistence::Record>, Error> {
        self.store.load()
    }

    // Reject before write or lose acknowledgement only after the actual durable write.
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
struct LedgerDisk {
    store: ledger::linux::FileStore,
    fault: Arc<AtomicU8>,
}
impl ledger::persistence::Persistence for LedgerDisk {
    // The native protected Ledger reconstructs its exact original accounting image.
    fn load(&mut self) -> Result<Option<ledger::persistence::Record>, ledger::Error> {
        self.store.load()
    }

    // A lost acknowledgement preserves the actual installed Ledger state before owner drop.
    fn commit(&mut self, value: &ledger::persistence::Record) -> Result<(), ledger::Error> {
        self.store.commit(value)?;
        if self.fault.swap(0, Ordering::SeqCst) == 2 {
            Err(ledger::Error::Unknown)
        } else {
            Ok(())
        }
    }
}
struct Dirs {
    storage: PathGuard,
    services: PathGuard,
    ledger: PathGuard,
}
struct Node {
    runtime: Coordinator<ServiceDisk>,
    account: Arc<ledger::state::Ledger<LedgerDisk>>,
    authority: Arc<Authority>,
    registry: Arc<services::registry::Registry>,
    ledger_authority: Arc<ledger::authority::Authority>,
    journal: Arc<Journal>,
    gate: Arc<Gate>,
    storage_faults: Arc<Faults>,
    services_fault: Arc<AtomicU8>,
    ledger_fault: Arc<AtomicU8>,
    inv: Invocation,
}
impl Node {
    // Reconstruct every authority, original key and participant from separate disk images after owner drop.
    fn open(dirs: &Dirs, create: bool, elapsed: u64) -> Self {
        let (inv, _, _) = contracts::store(false);
        let local = inv.provider;
        let store = Arc::new(Store::open(&dirs.storage.0, create).unwrap());
        let storage_faults = Arc::new(Faults {
            store: store.clone(),
            writes: Mutex::new(Default::default()),
        });
        let crypto = Arc::new(TestCrypto(std::sync::atomic::AtomicU8::new(7)));
        let journal = Arc::new(if create {
            Journal::initialize(
                local,
                storage_faults.clone(),
                crypto.clone(),
                image_limits(),
                Anchor::new(time(0)),
            )
            .unwrap()
        } else {
            Journal::open(
                local,
                storage_faults.clone(),
                crypto.clone(),
                image_limits(),
            )
            .unwrap()
        });
        let gate = Arc::new(Gate::new(journal.clone(), store, limits()).unwrap());
        let clock = Arc::new(TestClock(Mutex::new(time(elapsed))));
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
        let standards = Arc::new(Catalogue::new(Default::default()).unwrap());
        let client = Arc::new(
            Client::new(
                inv.definition.clone(),
                &standards,
                provider.clone(),
                clock,
                Policy {
                    values: B,
                    maximum_clock_difference_seconds: 60,
                },
            )
            .unwrap(),
        );
        let authority = Arc::new(Authority::new());
        let registry = Arc::new(
            services::registry::Registry::new(
                local,
                authority.clone(),
                Default::default(),
                standards,
            )
            .unwrap(),
        );
        let register = authority
            .grant(local, Action::Register, 16_777_216)
            .unwrap();
        let recovery = authority.grant(local, Action::Recover, 16_777_216).unwrap();
        registry
            .register(&context(&register), client, Some(provider))
            .unwrap();
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
        let ledger_fault = Arc::new(AtomicU8::new(0));
        let disk = LedgerDisk {
            store: ledger::linux::FileStore::open(&dirs.ledger.0, Default::default(), create)
                .unwrap(),
            fault: ledger_fault.clone(),
        };
        let account = Arc::new(if create {
            let lc = ledger_context(&admin);
            let ledger = ledger::state::Ledger::initialize(
                local,
                ledger_authority.clone(),
                disk,
                Default::default(),
                &lc,
                1000,
                100,
            )
            .unwrap();
            ledger.grant(&lc, inv.requester, 100).unwrap();
            ledger
                .set_trust(
                    &lc,
                    inv.requester,
                    ledger::accounting::Trust::new(1).unwrap(),
                )
                .unwrap();
            ledger
        } else {
            ledger::state::Ledger::open(
                local,
                ledger_authority.clone(),
                disk,
                Default::default(),
                &ledger_context(&read),
            )
            .unwrap()
        });
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
            &context(if create { &register } else { &recovery }),
            disk,
            accounting,
            registry.clone(),
            create,
        )
        .unwrap();
        Self {
            runtime,
            account,
            authority,
            registry,
            ledger_authority,
            journal,
            gate,
            storage_faults,
            services_fault,
            ledger_fault,
            inv,
        }
    }

    // Recover leases and pending obligations before allowing any new operation into the provider.
    fn recover(&self, elapsed: u64) {
        let grant = self
            .authority
            .grant(self.inv.provider, Action::Coordinate, 16_777_216)
            .unwrap();
        self.gate
            .recover(
                &Services {
                    coordinator: &self.runtime,
                    context: &context(&grant),
                },
                time(elapsed),
                None,
            )
            .unwrap();
    }

    // Build scoped local admission through a real current Board and exact quote acceptance.
    fn accepted(&self, input: &Value) -> services::quote::Accepted {
        let local = self.inv.provider;
        let authority = Arc::new(board::authority::Authority::new());
        let board = board::state::Provider::new(
            local,
            authority.clone(),
            Default::default(),
            self.registry.clone(),
        )
        .unwrap();
        let publish = authority
            .grant(local, board::authority::Action::Publish, 16_777_216)
            .unwrap();
        let select = authority
            .grant(local, board::authority::Action::Select, 16_777_216)
            .unwrap();
        let bp = board::contract::Context {
            grant: &publish,
            deadline: Instant::now() + Duration::from_secs(60),
            cancelled: false,
        };
        let bs = board::contract::Context {
            grant: &select,
            ..bp
        };
        let mut offering = self.inv.offering.source.clone();
        offering["creditIssuer"] = crate::modules::identity::text::format(local).into();
        let raw =
            serde_json::json!({"version":1,"providedServices":[offering],"wantedServices":[]});
        let definitions = BTreeMap::from([(
            self.inv.offering.id.clone(),
            self.inv.definition.document.bytes().to_vec(),
        )]);
        let prepared = board::prepared::Prepared::new(
            local,
            &serde_json::to_vec(&raw).unwrap(),
            &definitions,
            Default::default(),
            self.registry.as_ref(),
        )
        .unwrap();
        let snapshot = board.publish(&bp, prepared).unwrap();
        let selection = board
            .select(
                &bs,
                &snapshot,
                &board::selection::Request {
                    id: &self.inv.offering.id,
                    service: "cr2se.storage",
                    version: 1,
                    description: &self.inv.offering.description,
                    other: self.inv.requester,
                    authenticated: true,
                },
            )
            .unwrap();
        let invoke = self
            .authority
            .grant(local, Action::Invoke, 16_777_216)
            .unwrap();
        let context = context(&invoke);
        let peer = Peer {
            local,
            remote: self.inv.requester,
            key: self.inv.key,
        };
        let authorized = services::authorization::authorize(
            &context,
            &self.authority,
            local,
            &self.registry,
            selection,
            peer,
            None,
        )
        .unwrap();
        let assessed = services::assessment::assess(
            &context,
            &self.authority,
            authorized,
            input,
            Default::default(),
        )
        .unwrap();
        let quoter = Quoter::new(local, self.authority.clone(), Default::default()).unwrap();
        let quote = quoter
            .issue(
                &context,
                Issue {
                    assessed,
                    bytes: Blob::new(vec![8], 1).unwrap(),
                    maximum: 17,
                    expires: context.deadline,
                },
            )
            .unwrap();
        quoter
            .accept(
                &context,
                &quote,
                peer,
                quote.hash(),
                &BoardCurrent {
                    owner: &board,
                    context: &bs,
                },
            )
            .unwrap()
    }
}

// Every fresh coordinator call acquires the test's explicit versioned authority and deadline.
fn context(grant: &services::authority::Grant) -> Context<'_> {
    Context {
        version: 1,
        grant,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    }
}

// Give the accounting owner its independent exact local authority context.
fn ledger_context(grant: &ledger::authority::Grant) -> ledger::contract::Context<'_> {
    ledger::contract::Context {
        version: 1,
        grant,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    }
}

// Restart from each native durability cut, then replay recovery to assert exact one-time financial and byte effects.
#[test]
fn provider_reopens_every_coordination_boundary() {
    for cut in 0..8 {
        let dirs = Dirs {
            storage: path(),
            services: path(),
            ledger: path(),
        };
        let node = Node::open(&dirs, true, 20);
        node.recover(20);
        let (_, input, _) = contracts::store(false);
        let local = node.inv.provider;
        let key = node.inv.key;
        let grant = node
            .authority
            .grant(local, Action::Coordinate, 16_777_216)
            .unwrap();
        let context = context(&grant);
        if cut == 0 {
            node.ledger_fault.store(2, Ordering::SeqCst);
        }
        let admitted = node
            .runtime
            .admit(&context, node.accepted(&input), Payment::default());
        if cut == 0 {
            assert_eq!(admitted, Err(Error::Unknown));
        } else {
            assert_eq!(admitted, Ok(key));
        }
        if cut >= 2 {
            node.runtime.supply_input(&context, key, input).unwrap();
            node.runtime.start(&context, key).unwrap();
            node.runtime.stage(&context, key).unwrap();
            if cut == 2 {
                node.storage_faults
                    .writes
                    .lock()
                    .unwrap()
                    .push_back(Write::LoseReply);
            }
            if cut == 3 {
                node.ledger_fault.store(2, Ordering::SeqCst);
            }
            let prepared = node.runtime.prepare(&context, key);
            assert_eq!(
                prepared,
                if cut <= 3 {
                    Err(Error::Unknown)
                } else {
                    Ok(())
                }
            );
        }
        if cut >= 5 {
            node.runtime
                .decide(&context, key, Decision::Commit)
                .unwrap();
        }
        if cut == 6 {
            node.storage_faults
                .writes
                .lock()
                .unwrap()
                .push_back(Write::LoseReply);
            assert_eq!(node.runtime.commit(&context, key), Err(Error::Unknown));
        }
        if cut == 7 {
            node.services_fault.store(1, Ordering::SeqCst);
            assert_eq!(node.runtime.commit(&context, key), Err(Error::Storage));
        }
        drop(node);

        let node = Node::open(&dirs, false, 22);
        node.recover(22);
        node.recover(22);
        let observe = node
            .authority
            .grant(local, Action::Observe, 16_777_216)
            .unwrap();
        let oc = self::context(&observe);
        let read = node
            .ledger_authority
            .grant(local, ledger::authority::Action::Read, 16_777_216)
            .unwrap();
        let issued = node
            .account
            .view(&ledger_context(&read), key.requester)
            .unwrap()
            .issued;
        let image = node.journal.snapshot().unwrap();
        if cut < 2 {
            assert_eq!(node.runtime.status(&oc, key), Ok(Some(Phase::Aborted)));
            assert_eq!(issued, 100);
            assert!(image.leases.is_empty());
            assert!(node.runtime.result(&oc, key).is_err());
        } else {
            assert_eq!(node.runtime.status(&oc, key), Ok(Some(Phase::Committed)));
            assert_eq!(issued, 83);
            assert_eq!(image.leases.len(), 1);
            let accepted = if cut >= 5 { 1020 } else { 1022 };
            let result = node.runtime.result(&oc, key).unwrap();
            assert_eq!(renewal::number(&result.output, "acceptedAt"), Ok(accepted));
            assert_eq!(
                renewal::number(&result.output, "expiresAt"),
                Ok(accepted + 10)
            );
            let lease = &image.leases[0];
            assert_eq!(lease.state, State::Active);
            assert_eq!(lease.accepted_at, accepted);
            assert_eq!(
                node.gate
                    .byte_store()
                    .reopen(lease.id, 3)
                    .unwrap()
                    .read(0, 3, 3)
                    .unwrap(),
                b"abc"
            );
        }
    }
}
