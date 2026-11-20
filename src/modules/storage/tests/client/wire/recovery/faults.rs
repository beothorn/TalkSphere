//! One-shot before-commit refusal and after-fsync lost replies wrap real independent native stores, never the protocol implementation.
use super::*;
use std::sync::atomic::AtomicU8;
struct ServicesStore {
    native: services::linux::FileStore,
    cut: Arc<AtomicU8>,
}
struct LedgerStore {
    native: ledger::linux::FileStore,
    cut: Arc<AtomicU8>,
}
impl services::persistence::Persistence for ServicesStore {
    // Recovery reads the real synchronized complete image; no cloned in-memory state can repair or replace it.
    fn load(&mut self) -> Result<Option<services::persistence::Record>, Error> {
        self.native.load()
    }
    // Mode one refuses before any write; mode two loses the reply only after the real native durability acknowledgement.
    fn commit(&mut self, record: &services::persistence::Record) -> Result<(), Error> {
        let cut = self.cut.swap(0, Ordering::SeqCst);
        if cut == 1 {
            return Err(Error::Storage);
        }
        self.native.commit(record)?;
        if cut == 2 {
            return Err(Error::Unknown);
        }
        Ok(())
    }
}
impl ledger::persistence::Persistence for LedgerStore {
    // Ledger loads its independently owned file through its public adapter, without inspecting Services or Storage records.
    fn load(&mut self) -> Result<Option<ledger::persistence::Record>, ledger::Error> {
        self.native.load()
    }
    // The participant's one-shot cut is independent of coordinator cuts and retains the actual file after an uncertain reply.
    fn commit(&mut self, record: &ledger::persistence::Record) -> Result<(), ledger::Error> {
        let cut = self.cut.swap(0, Ordering::SeqCst);
        if cut == 1 {
            return Err(ledger::Error::Storage);
        }
        self.native.commit(record)?;
        if cut == 2 {
            return Err(ledger::Error::Unknown);
        }
        Ok(())
    }
}
struct Opened {
    endpoint: Endpoint<LedgerStore, ServicesStore>,
    service_cut: Arc<AtomicU8>,
    ledger_cut: Arc<AtomicU8>,
}
// Every reopen creates new fault controls, grants, codecs and owners and obtains state solely from the two independent protected files.
fn open_faults(path: &Path, inv: &Invocation, initialize: bool) -> Opened {
    let service_cut = Arc::new(AtomicU8::new(0));
    let ledger_cut = Arc::new(AtomicU8::new(0));
    let endpoint = endpoint_with_stores(
        fixture().client,
        inv,
        inv.provider,
        Payment::default(),
        LedgerStore {
            native: ledger::linux::FileStore::open(
                &path.join("ledger"),
                Default::default(),
                initialize,
            )
            .unwrap(),
            cut: ledger_cut.clone(),
        },
        ServicesStore {
            native: services::linux::FileStore::open(
                &path.join("services"),
                Default::default(),
                initialize,
            )
            .unwrap(),
            cut: service_cut.clone(),
        },
        initialize,
    );
    Opened {
        endpoint,
        service_cut,
        ledger_cut,
    }
}

// Preparation/decision/charge/final acknowledgement failures recover the original operation exactly once, including uncertain writes and confirmed no-write refusal.
#[test]
fn native_independent_owner_failures_reconcile_without_duplicate_charge() {
    let (inv, input, output) = store_fixture(false);
    for cut in 0..8 {
        let directory = Directory::new();
        let opened = open_faults(&directory.0, &inv, true);
        let e = &opened.endpoint;
        let outgoing = e.reserve(input.clone(), 17, json!({}));
        stage(e, &outgoing, &output, 17);
        let grant = coordinate(e);
        let (error, phase) = match cut {
            0 | 1 | 6 => {
                let target = if cut == 1 {
                    &opened.ledger_cut
                } else {
                    &opened.service_cut
                };
                target.store(if cut == 6 { 1 } else { 2 }, Ordering::SeqCst);
                (
                    e.runtime.prepare(&sc(&grant), inv.key).unwrap_err(),
                    if cut == 0 {
                        Phase::Prepared
                    } else {
                        Phase::Staged
                    },
                )
            }
            2 | 7 => {
                e.runtime.prepare(&sc(&grant), inv.key).unwrap();
                opened
                    .service_cut
                    .store(if cut == 7 { 1 } else { 2 }, Ordering::SeqCst);
                (
                    e.runtime
                        .decide(&sc(&grant), inv.key, Decision::Commit)
                        .unwrap_err(),
                    if cut == 2 {
                        Phase::Commit
                    } else {
                        Phase::Prepared
                    },
                )
            }
            _ => {
                e.runtime.prepare(&sc(&grant), inv.key).unwrap();
                e.runtime
                    .decide(&sc(&grant), inv.key, Decision::Commit)
                    .unwrap();
                let target = if cut == 4 {
                    &opened.service_cut
                } else {
                    &opened.ledger_cut
                };
                target.store(if cut == 5 { 1 } else { 2 }, Ordering::SeqCst);
                (
                    e.runtime.commit(&sc(&grant), inv.key).unwrap_err(),
                    if cut == 4 {
                        Phase::Committed
                    } else {
                        Phase::Commit
                    },
                )
            }
        };
        assert_eq!(
            error,
            if cut >= 5 {
                Error::Storage
            } else {
                Error::Unknown
            }
        );
        // Ambiguous final acknowledgement cannot leak a successful result from the still-open uncertain owner.
        assert!(completed(e).is_err());
        drop(outgoing);
        drop(opened);
        let opened = open_faults(&directory.0, &inv, false);
        let e = &opened.endpoint;
        assert_eq!(e.phase(), Some(phase));
        assert_eq!(e.available(), CREDIT - 17);
        let grant = coordinate(e);
        assert_eq!(
            e.runtime.resume(&sc(&grant), inv.key).unwrap(),
            Phase::Committed
        );
        assert_eq!(
            e.runtime.resume(&sc(&grant), inv.key).unwrap(),
            Phase::Committed
        );
        let result = completed(e).unwrap();
        same_output(result.output, &output);
        assert_eq!(result.price, 17);
        let read = e
            .ledger_authority
            .grant(inv.requester, ledger::authority::Action::Read, 65536)
            .unwrap();
        assert_eq!(
            e.ledger.view(&lc(&read), inv.provider).unwrap().owned,
            CREDIT - 17
        );
        assert_eq!(e.available(), CREDIT - 17);
        drop(opened);
        let reopened = open_faults(&directory.0, &inv, false);
        assert_eq!(reopened.endpoint.available(), CREDIT - 17);
        assert_eq!(reopened.endpoint.phase(), Some(Phase::Committed));
        same_output(completed(&reopened.endpoint).unwrap().output, &output);
    }
}
