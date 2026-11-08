//! Executable Ledger inspection syntax, help, exact accounting and failure discipline.
use std::{
    path::Path,
    process::{Command, Output},
};
// Every command runs the real binary with stable explicit logging configuration.
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_talksphere"))
        .args(args)
        .env("RUST_LOG", "off")
        .output()
        .unwrap()
}
// New option spellings all lead to detailed help and invalid syntax cannot initialize state.
#[test]
fn help_and_syntax() {
    for topic in [
        "ledger-show",
        "--ledger-store",
        "--local-identity",
        "--account",
    ] {
        let o = run(&["help", topic]);
        assert!(o.status.success());
        let text = String::from_utf8(o.stdout).unwrap();
        assert!(text.contains("--ledger-store DIRECTORY"));
        assert!(text.contains("16777216"));
        assert!(o.stderr.is_empty());
    }
    assert!(run(&["ledger-show", "--help"]).status.success());
    for args in [
        vec!["ledger-show"],
        vec!["ledger-show", "--ledger-store", "missing"],
        vec![
            "ledger-show",
            "--ledger-store",
            "missing",
            "--local-identity",
            "bad",
            "--account",
            "bad",
        ],
    ] {
        let o = run(&args);
        assert_eq!(o.status.code(), Some(2));
        assert!(!o.stderr.is_empty());
        assert!(o.stdout.is_empty());
    }
}
// Inspect a real protected accounting image without changing durable bytes; missing/corrupt inputs stay failures.
#[cfg(target_os = "linux")]
#[test]
fn inspect_existing_state() {
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    use talksphere::modules::{
        identity::{api::IdentityId, text},
        ledger::{
            authority::{Action, Authority},
            contract::{Context, Limits},
            linux::FileStore,
            state::Ledger,
        },
    };
    let path = std::env::temp_dir().join(format!("talksphere-ledger-cli-{}", std::process::id()));
    let local = IdentityId([1; 32]);
    let peer = IdentityId([2; 32]);
    let local_text = text::format(local);
    let peer_text = text::format(peer);
    let auth = Arc::new(Authority::new());
    let admin = auth.grant(local, Action::Administer, 1_000_000).unwrap();
    let context = Context {
        version: 1,
        grant: &admin,
        deadline: Instant::now() + Duration::from_secs(30),
        cancelled: false,
    };
    let limits = Limits::default();
    let ledger = Ledger::initialize(
        local,
        auth,
        FileStore::open(&path, limits, true).unwrap(),
        limits,
        &context,
        100,
        10,
    )
    .unwrap();
    ledger.grant(&context, peer, 7).unwrap();
    ledger.set_owned(&context, peer, u64::MAX).unwrap();
    let args = [
        "ledger-show",
        "--ledger-store",
        path.to_str().unwrap(),
        "--local-identity",
        &local_text,
        "--account",
        &peer_text,
    ];
    assert_eq!(run(&args).status.code(), Some(1));
    drop(ledger);
    let before = std::fs::read(path.join("ledger.bin")).unwrap();
    let output = run(&args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = String::from_utf8(output.stdout).unwrap();
    assert!(output.contains("owned_from_account=18446744073709551615"));
    assert!(output.contains("issued_to_account=7"));
    assert!(output.contains("issuance_available=93"));
    assert!(output.contains("unknown_payments=0"));
    assert_eq!(before, std::fs::read(path.join("ledger.bin")).unwrap());
    let mut wrong = args;
    wrong[4] = &peer_text;
    assert_eq!(run(&wrong).status.code(), Some(1));
    std::fs::write(path.join("ledger.bin"), b"corrupt").unwrap();
    let bad = run(&args);
    assert_eq!(bad.status.code(), Some(1));
    assert!(!bad.stderr.is_empty());
    assert!(bad.stdout.is_empty());
    assert_eq!(std::fs::read(path.join("ledger.bin")).unwrap(), b"corrupt");
    std::fs::remove_dir_all(&path).unwrap();
    let missing = run(&args);
    assert_eq!(missing.status.code(), Some(1));
    assert!(!Path::new(&path).exists());
}
