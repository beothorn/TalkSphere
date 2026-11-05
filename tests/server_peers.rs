//! Independent executable peers authenticate configured contacts in both TCP roles across restarts.
#![cfg(target_os = "linux")]
#[path = "support/runtime.rs"]
mod runtime;
use runtime::ProcessFixture;
use std::{net::SocketAddr, path::Path, process::Command};
use talksphere::modules::identity::{api::Contact, contact, text};
// Initialize a distinct real key store and return its public canonical identity.
fn initialize(path: &Path) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_talksphere"))
        .args(["identity-init", "--identity-store"])
        .arg(path)
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap().trim().into()
}
// Start one real server and require complete readiness before using its actual endpoint.
fn start(path: &Path, pinned: Option<(&str, SocketAddr)>) -> (ProcessFixture, SocketAddr) {
    let mut args = vec![
        "serve".to_string(),
        "--identity-store".into(),
        path.to_str().unwrap().into(),
    ];
    if let Some((id, endpoint)) = pinned {
        args.extend([
            "--contact".into(),
            contact::format(Contact {
                expected_id: text::parse(id).unwrap(),
                endpoint: Some(endpoint),
            })
            .unwrap(),
        ]);
    }
    let process = ProcessFixture::spawn(&args.iter().map(String::as_str).collect::<Vec<_>>());
    let ready = process.line().unwrap();
    let fields: Vec<_> = ready.split_whitespace().collect();
    assert_eq!(fields[0], "ready");
    (process, fields[2].parse().unwrap())
}
// Drain bounded public lifecycle observations until the expected terminal status and reap the child.
fn stop(process: &mut ProcessFixture) {
    process.signal(rustix::process::Signal::TERM).unwrap();
    for _ in 0..16 {
        if process.line().unwrap().trim() == "stopped" {
            assert!(process.finish().unwrap().success());
            return;
        }
    }
    panic!("server did not report stopped within its bounded lifecycle observations");
}
// Restart both protected identities with reversed TCP roles and require the same exact public IDs.
#[test]
fn independent_processes_authenticate_in_both_directions() {
    let fixture = ProcessFixture::spawn(&["--version"]);
    assert!(fixture.line().is_ok());
    let a = fixture.root.join("a");
    let b = fixture.root.join("b");
    let a_id = initialize(&a);
    let b_id = initialize(&b);
    assert_ne!(a_id, b_id);
    for reverse in [false, true] {
        let (listener_store, listener_id, caller_store, caller_id) = if reverse {
            (&a, &a_id, &b, &b_id)
        } else {
            (&b, &b_id, &a, &a_id)
        };
        let (mut listener, address) = start(listener_store, None);
        let (mut caller, _) = start(caller_store, Some((listener_id, address)));
        assert_eq!(
            caller.line().unwrap().trim(),
            format!("authenticated {listener_id} Initiator")
        );
        assert_eq!(
            listener.line().unwrap().trim(),
            format!("authenticated {caller_id} Acceptor")
        );
        stop(&mut caller);
        stop(&mut listener);
    }
    let invalid = ProcessFixture::spawn(&["--unknown"]);
    assert!(invalid.error().is_ok());
}
