//! Fresh-process Ledger reopen proves acknowledged participant state survives process lifetime.
#![cfg(target_os = "linux")]
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use talksphere::modules::{
    identity::api::IdentityId,
    ledger::{
        authority::{Action, Authority, Grant},
        contract::{Context, Limits, OperationKey},
        linux::FileStore,
        reservation::Status,
        state::Ledger,
        terms::{Policy, Terms},
    },
};
// Borrow one explicit capability with an independent monotonic deadline.
fn context(g: &Grant) -> Context<'_> {
    Context {
        version: 1,
        grant: g,
        deadline: Instant::now() + Duration::from_secs(30),
        cancelled: false,
    }
}
// Use identical finite limits on both sides of the actual process boundary.
fn limits() -> Limits {
    Limits {
        accounts: 8,
        operations: 8,
        transfers: 8,
        claims: 8,
        quotes: 8,
        state_bytes: 65536,
    }
}
// A child opens only durable state and queries status; it has no parent heap or descriptors.
#[test]
fn child_reopen() {
    let Some(path) = std::env::var_os("TALKSPHERE_LEDGER_REOPEN") else {
        return;
    };
    let a = IdentityId([1; 32]);
    let auth = Arc::new(Authority::new());
    let read = auth.grant(a, Action::Read, 65536).unwrap();
    let coord = auth.grant(a, Action::Coordinate, 65536).unwrap();
    let ledger = Ledger::open(
        a,
        auth,
        FileStore::open(std::path::Path::new(&path), limits(), false).unwrap(),
        limits(),
        &context(&read),
    )
    .unwrap();
    assert_eq!(
        ledger.status(
            &context(&coord),
            OperationKey {
                requester: a,
                connection: [3; 32],
                stream: 1
            }
        ),
        Ok(Some(Status::Committed))
    );
    assert_eq!(
        ledger
            .view(&context(&read), IdentityId([2; 32]))
            .unwrap()
            .owned,
        93
    );
}
// Parent acknowledges one commit then exits its writer scope before spawning the fresh reader.
#[test]
fn committed_state_survives_process_reopen() {
    let path =
        std::env::temp_dir().join(format!("talksphere-ledger-process-{}", std::process::id()));
    let a = IdentityId([1; 32]);
    let b = IdentityId([2; 32]);
    let auth = Arc::new(Authority::new());
    let admin = auth.grant(a, Action::Administer, 65536).unwrap();
    let coord = auth.grant(a, Action::Coordinate, 65536).unwrap();
    let ledger = Ledger::initialize(
        a,
        auth,
        FileStore::open(&path, limits(), true).unwrap(),
        limits(),
        &context(&admin),
        100,
        100,
    )
    .unwrap();
    ledger.set_owned(&context(&admin), b, 100).unwrap();
    let key = OperationKey {
        requester: a,
        connection: [3; 32],
        stream: 1,
    };
    let r = ledger
        .reserve(
            &context(&coord),
            key,
            Terms {
                requester: a,
                provider: b,
                issuer: b,
                maximum: 7,
                fixed: true,
                accepted: vec![9],
                transfer: None,
                fee: None,
                spending_bound: 7,
            },
            Policy::default(),
        )
        .unwrap();
    let p = ledger.prepare(&context(&coord), &r, 7, vec![1]).unwrap();
    ledger.commit(&context(&coord), &p).unwrap();
    drop(ledger);
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_reopen"])
        .env("TALKSPHERE_LEDGER_REOPEN", &path)
        .status()
        .unwrap();
    assert!(result.success());
    std::fs::remove_dir_all(path).unwrap();
}

// A separately executed process cannot acquire a store while its authoritative writer remains live.
#[test]
fn child_writer_contention(){let Some(path)=std::env::var_os("TALKSPHERE_LEDGER_LOCK")else{return};assert!(FileStore::open(std::path::Path::new(&path),limits(),false).is_err());}
// CLOEXEC prevents inherited descriptors from creating a false positive in the child lock check.
#[test]
fn exclusive_writer_across_processes(){let path=std::env::temp_dir().join(format!("talksphere-ledger-lock-{}",std::process::id()));let store=FileStore::open(&path,limits(),true).unwrap();let result=std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact","child_writer_contention"]).env("TALKSPHERE_LEDGER_LOCK",&path).status().unwrap();assert!(result.success());drop(store);std::fs::remove_dir_all(path).unwrap();}
