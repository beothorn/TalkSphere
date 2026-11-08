//! SV-09 restarts discard module heaps and recover only complete durable byte images.
use super::{
    Error, accounting,
    authority::{Action, Authority},
    contract::{Limits, OperationKey},
    handler::{Provider, Status},
    registry::Registry,
    runtime::{Config, Coordinator, Decision, Payment, Phase},
    runtime_tests::{accepted, context},
    test_support::{Cut, durable_family::Durable, selection::selected_ports, stores},
};
use crate::modules::{identity::api::IdentityId, ledger};
use std::sync::{Arc, Mutex};
struct Harness {
    runtime: Coordinator<stores::Services>,
    authority: Arc<Authority>,
    registry: Arc<Registry>,
    ledger: Arc<ledger::state::Ledger<stores::Ledger>>,
    ledger_authority: Arc<ledger::authority::Authority>,
    family: Arc<Durable>,
}
// Each restart constructs new authorities, implementations and module state from byte stores only.
fn open(
    svc: stores::Services,
    accounts: stores::Ledger,
    image: Arc<Mutex<Vec<u8>>>,
    initialize: bool,
) -> Harness {
    let local = IdentityId([2; 32]);
    let requester = IdentityId([1; 32]);
    let authority = Arc::new(Authority::new());
    let family = Arc::new(Durable::open(image).unwrap());
    let (_, _, _, registry) =
        selected_ports(false, family.clone(), family.clone(), authority.clone());
    let ledger_authority = Arc::new(ledger::authority::Authority::new());
    let action = if initialize {
        ledger::authority::Action::Administer
    } else {
        ledger::authority::Action::Read
    };
    let grant = ledger_authority.grant(local, action, 1_000_000).unwrap();
    let lg = ledger::contract::Context {
        version: 1,
        grant: &grant,
        deadline: std::time::Instant::now() + std::time::Duration::from_secs(60),
        cancelled: false,
    };
    let ledger = Arc::new(if initialize {
        let l = ledger::state::Ledger::initialize(
            local,
            ledger_authority.clone(),
            accounts,
            Default::default(),
            &lg,
            1000,
            100,
        )
        .unwrap();
        l.grant(&lg, requester, 100).unwrap();
        l.set_trust(&lg, requester, ledger::accounting::Trust::new(1).unwrap())
            .unwrap();
        l
    } else {
        ledger::state::Ledger::open(
            local,
            ledger_authority.clone(),
            accounts,
            Default::default(),
            &lg,
        )
        .unwrap()
    });
    let grant = ledger_authority
        .grant(local, ledger::authority::Action::Coordinate, 1_000_000)
        .unwrap();
    let accounting = Arc::new(accounting::Ledger::new(
        ledger.clone(),
        grant,
        Default::default(),
    ));
    let grant = authority
        .grant(
            local,
            if initialize {
                Action::Register
            } else {
                Action::Recover
            },
            1_000_000,
        )
        .unwrap();
    let runtime = Coordinator::open(
        Config {
            local,
            authority: authority.clone(),
            limits: Limits::default(),
        },
        &context(&grant),
        svc,
        accounting,
        registry.clone(),
        initialize,
    )
    .unwrap();
    Harness {
        runtime,
        authority,
        registry,
        ledger,
        ledger_authority,
        family,
    }
}
// Crash at every normative boundary; no committed decision is replaced and execution never recovers a reply.
#[test]
fn durable_coordination_fault_matrix() {
    let local = IdentityId([2; 32]);
    let key = OperationKey {
        requester: IdentityId([1; 32]),
        connection: [8; 32],
        stream: 1,
    };
    for cut in 0..8 {
        let svc = stores::Services::default();
        let accounts = stores::Ledger::default();
        let image = Arc::new(Mutex::new(Vec::new()));
        let h = open(svc.clone(), accounts.clone(), image.clone(), true);
        let grant = h
            .authority
            .grant(local, Action::Coordinate, 1_000_000)
            .unwrap();
        let c = context(&grant);
        if cut == 0 {
            accounts.script([Cut::After]);
            assert_eq!(
                h.runtime.admit(
                    &c,
                    accepted(h.authority.clone(), &h.registry, 1, 1),
                    Payment::default()
                ),
                Err(Error::Unknown)
            );
        } else {
            h.runtime
                .admit(
                    &c,
                    accepted(h.authority.clone(), &h.registry, 1, 1),
                    Payment::default(),
                )
                .unwrap();
            h.runtime
                .supply_input(&c, key, super::value::Value::String("input".into()))
                .unwrap();
            if cut >= 2 {
                h.runtime.start(&c, key).unwrap();
                h.runtime.stage(&c, key).unwrap();
                match cut {
                    2 => {
                        svc.script([Cut::After]);
                        assert_eq!(h.runtime.prepare(&c, key), Err(Error::Unknown));
                    }
                    3 => {
                        svc.script([Cut::None, Cut::After]);
                        assert_eq!(h.runtime.prepare(&c, key), Err(Error::Unknown));
                    }
                    _ => {
                        h.runtime.prepare(&c, key).unwrap();
                        if cut == 4 {
                            svc.script([Cut::Before]);
                            assert_eq!(
                                h.runtime.decide(&c, key, Decision::Commit),
                                Err(Error::Storage)
                            );
                        } else if cut == 5 {
                            svc.script([Cut::After]);
                            assert_eq!(
                                h.runtime.decide(&c, key, Decision::Commit),
                                Err(Error::Unknown)
                            );
                        } else {
                            h.runtime.decide(&c, key, Decision::Commit).unwrap();
                            if cut == 6 {
                                accounts.script([Cut::Before]);
                                assert_eq!(h.runtime.commit(&c, key), Err(Error::Storage));
                            } else {
                                svc.script([Cut::After]);
                                assert_eq!(h.runtime.commit(&c, key), Err(Error::Unknown));
                            }
                        }
                    }
                }
            }
        }
        drop(h);
        let h = open(svc, accounts, image, false);
        let fresh = h
            .authority
            .grant(local, Action::Coordinate, 1_000_000)
            .unwrap();
        let recovered = context(&fresh);
        assert_eq!(h.runtime.reserve(&c, key), Err(Error::Denied));
        if cut <= 1 {
            if cut == 0 {
                h.runtime.reserve(&recovered, key).unwrap();
            }
            h.runtime.decide(&recovered, key, Decision::Abort).unwrap();
            h.runtime.abort(&recovered, key).unwrap();
        } else {
            assert_eq!(h.runtime.resume(&recovered, key), Ok(Phase::Committed));
            assert_eq!(h.runtime.resume(&recovered, key), Ok(Phase::Committed));
            assert_eq!(
                h.runtime.decide(&recovered, key, Decision::Abort),
                Err(Error::Conflict)
            );
        }
        let observe = h
            .authority
            .grant(local, Action::Observe, 1_000_000)
            .unwrap();
        let phase = h.runtime.status(&context(&observe), key).unwrap();
        let read = h
            .ledger_authority
            .grant(local, ledger::authority::Action::Read, 1_000_000)
            .unwrap();
        let lc = ledger::contract::Context {
            version: 1,
            grant: &read,
            deadline: recovered.deadline,
            cancelled: false,
        };
        if cut <= 1 {
            assert_eq!(phase, Some(Phase::Aborted));
            assert_eq!(h.ledger.view(&lc, key.requester).unwrap().issued, 100);
            assert!(!matches!(h.family.status(key).unwrap(), Status::Committed));
            assert_eq!(h.family.executions(), 0);
        } else {
            assert_eq!(phase, Some(Phase::Committed));
            assert_eq!(h.ledger.view(&lc, key.requester).unwrap().issued, 93);
            assert!(matches!(h.family.status(key).unwrap(), Status::Committed));
            assert_eq!(h.family.executions(), 1);
            let result = h.runtime.result(&context(&observe), key).unwrap();
            assert_eq!(result.price, 7);
            assert_eq!(result.result.bytes(), b"input");
        }
    }
}

#[cfg(target_os = "linux")]
struct DiskHarness {
    runtime: Coordinator<super::linux::FileStore>,
    authority: Arc<Authority>,
    ledger: Arc<ledger::state::Ledger<ledger::linux::FileStore>>,
    ledger_authority: Arc<ledger::authority::Authority>,
    family: Arc<Durable>,
}
// Fresh processes open independently protected stores and reconstruct every participant without shared heaps.
#[cfg(target_os = "linux")]
fn disk_open(path: &std::path::Path) -> DiskHarness {
    let local = IdentityId([2; 32]);
    let authority = Arc::new(Authority::new());
    let family = Arc::new(Durable::open_disk(&path.join("family")).unwrap());
    let (_, _, _, registry) =
        selected_ports(false, family.clone(), family.clone(), authority.clone());
    let ledger_authority = Arc::new(ledger::authority::Authority::new());
    let read = ledger_authority
        .grant(local, ledger::authority::Action::Read, 1_000_000)
        .unwrap();
    let lc = ledger::contract::Context {
        version: 1,
        grant: &read,
        deadline: std::time::Instant::now() + std::time::Duration::from_secs(60),
        cancelled: false,
    };
    let ledger = Arc::new(
        ledger::state::Ledger::open(
            local,
            ledger_authority.clone(),
            ledger::linux::FileStore::open(&path.join("ledger"), Default::default(), false)
                .unwrap(),
            Default::default(),
            &lc,
        )
        .unwrap(),
    );
    let grant = ledger_authority
        .grant(local, ledger::authority::Action::Coordinate, 1_000_000)
        .unwrap();
    let accounting = Arc::new(accounting::Ledger::new(
        ledger.clone(),
        grant,
        Default::default(),
    ));
    let recover = authority.grant(local, Action::Recover, 1_000_000).unwrap();
    let runtime = Coordinator::open(
        Config {
            local,
            authority: authority.clone(),
            limits: Limits::default(),
        },
        &context(&recover),
        super::linux::FileStore::open(&path.join("services"), Default::default(), false).unwrap(),
        accounting,
        registry,
        false,
    )
    .unwrap();
    DiskHarness {
        runtime,
        authority,
        ledger,
        ledger_authority,
        family,
    }
}
// The subprocess receives only a filesystem location and must recover the original recorded commit.
#[test]
#[cfg(target_os = "linux")]
fn reopen_child() {
    let Some(path) = std::env::var_os("TALKSPHERE_SERVICES_RECOVERY_CASE") else {
        return;
    };
    let h = disk_open(std::path::Path::new(&path));
    let grant = h
        .authority
        .grant(IdentityId([2; 32]), Action::Coordinate, 1_000_000)
        .unwrap();
    let key = OperationKey {
        requester: IdentityId([1; 32]),
        connection: [8; 32],
        stream: 1,
    };
    assert_eq!(
        h.runtime.resume(&context(&grant), key),
        Ok(Phase::Committed)
    );
    assert_eq!(h.family.executions(), 1);
}
// A separately exec'd test process commits the remaining Ledger effect and leaves all durable images reopenable.
#[test]
#[cfg(target_os = "linux")]
fn fresh_process_recovers_pending_commit() {
    use super::persistence::Persistence;
    use ledger::persistence::Persistence as LedgerPersistence;
    use std::os::unix::fs::PermissionsExt;
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "talksphere-services-recovery-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    std::fs::create_dir(&root).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    let svc = stores::Services::default();
    let accounts = stores::Ledger::default();
    let image = Arc::new(Mutex::new(Vec::new()));
    let h = open(svc.clone(), accounts.clone(), image.clone(), true);
    let local = IdentityId([2; 32]);
    let cg = h
        .authority
        .grant(local, Action::Coordinate, 1_000_000)
        .unwrap();
    let c = context(&cg);
    let key = h
        .runtime
        .admit(
            &c,
            accepted(h.authority.clone(), &h.registry, 1, 1),
            Payment::default(),
        )
        .unwrap();
    h.runtime
        .supply_input(&c, key, super::value::Value::String("input".into()))
        .unwrap();
    h.runtime.start(&c, key).unwrap();
    h.runtime.stage(&c, key).unwrap();
    h.runtime.prepare(&c, key).unwrap();
    h.runtime.decide(&c, key, Decision::Commit).unwrap();
    accounts.script([Cut::Before]);
    assert_eq!(h.runtime.commit(&c, key), Err(Error::Storage));
    drop(h);
    let mut service_store =
        super::linux::FileStore::open(&root.join("services"), Default::default(), true).unwrap();
    service_store.commit(&svc.image().unwrap()).unwrap();
    drop(service_store);
    let mut ledger_store =
        ledger::linux::FileStore::open(&root.join("ledger"), Default::default(), true).unwrap();
    ledger_store.commit(&accounts.image().unwrap()).unwrap();
    drop(ledger_store);
    let mut family_store =
        super::linux::FileStore::open(&root.join("family"), Default::default(), true).unwrap();
    family_store
        .commit(&super::persistence::Record {
            identity: local,
            payload: image.lock().unwrap().clone(),
        })
        .unwrap();
    drop(family_store);
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "modules::services::recovery_tests::reopen_child"])
        .env("TALKSPHERE_SERVICES_RECOVERY_CASE", &root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let h = disk_open(&root);
    let observe = h
        .authority
        .grant(local, Action::Observe, 1_000_000)
        .unwrap();
    assert_eq!(h.runtime.result(&context(&observe), key).unwrap().price, 7);
    assert_eq!(h.family.executions(), 1);
    let read = h
        .ledger_authority
        .grant(local, ledger::authority::Action::Read, 1_000_000)
        .unwrap();
    let lc = ledger::contract::Context {
        version: 1,
        grant: &read,
        deadline: context(&observe).deadline,
        cancelled: false,
    };
    assert_eq!(h.ledger.view(&lc, key.requester).unwrap().issued, 93);
    drop(h);
    std::fs::remove_dir_all(root).unwrap();
}

// Missing provider recovery code gates startup instead of substituting a requester-only codec for accepted obligations.
#[test]
fn missing_recovery_provider_is_explicit() {
    let svc = stores::Services::default();
    let accounts = stores::Ledger::default();
    let image = Arc::new(Mutex::new(Vec::new()));
    let h = open(svc.clone(), accounts.clone(), image, true);
    let local = IdentityId([2; 32]);
    let grant = h
        .authority
        .grant(local, Action::Coordinate, 1_000_000)
        .unwrap();
    let c = context(&grant);
    h.runtime
        .admit(
            &c,
            accepted(h.authority.clone(), &h.registry, 1, 1),
            Payment::default(),
        )
        .unwrap();
    drop(h);
    let authority = Arc::new(Authority::new());
    let requester = Arc::new(super::test_support::family::Echo::new());
    let registry = Arc::new(
        Registry::new(
            local,
            authority.clone(),
            Limits::default(),
            Arc::new(super::registry::NoStandards),
        )
        .unwrap(),
    );
    let registration = authority.grant(local, Action::Register, 1_000_000).unwrap();
    registry
        .register(&context(&registration), requester, None)
        .unwrap();
    let la = Arc::new(ledger::authority::Authority::new());
    let read = la
        .grant(local, ledger::authority::Action::Read, 1_000_000)
        .unwrap();
    let lc = ledger::contract::Context {
        version: 1,
        grant: &read,
        deadline: c.deadline,
        cancelled: false,
    };
    let ledger = Arc::new(
        ledger::state::Ledger::open(local, la.clone(), accounts, Default::default(), &lc).unwrap(),
    );
    let coordinate = la
        .grant(local, ledger::authority::Action::Coordinate, 1_000_000)
        .unwrap();
    let adapter = Arc::new(accounting::Ledger::new(
        ledger.clone(),
        coordinate,
        Default::default(),
    ));
    let recovery = authority.grant(local, Action::Recover, 1_000_000).unwrap();
    assert!(matches!(
        Coordinator::open(
            Config {
                local,
                authority: authority.clone(),
                limits: Limits::default()
            },
            &context(&recovery),
            svc,
            adapter,
            registry,
            false
        ),
        Err(Error::Unavailable)
    ));
    assert_eq!(ledger.view(&lc, IdentityId([1; 32])).unwrap().issued, 100);
}
