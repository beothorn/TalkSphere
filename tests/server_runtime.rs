//! Actual executable startup and graceful termination use protected credentials and real socket binding.
#![cfg(target_os = "linux")]
#[path = "support/runtime.rs"]
mod runtime;
use std::process::Command;
// Ready reports the actual address; SIGTERM completes normally and releases the port before Stopped is reported.
#[test]
fn configured_server_starts_and_stops_cleanly() {
    let fixture = runtime::ProcessFixture::spawn(&["--version"]);
    assert!(fixture.line().is_ok());
    let path = fixture.root.join("identity");
    assert!(
        Command::new(env!("CARGO_BIN_EXE_talksphere"))
            .args(["identity-init", "--identity-store"])
            .arg(&path)
            .output()
            .unwrap()
            .status
            .success()
    );
    let mut server =
        runtime::ProcessFixture::spawn(&["serve", "--identity-store", path.to_str().unwrap()]);
    let ready = server.line().unwrap();
    let fields: Vec<_> = ready.split_whitespace().collect();
    assert_eq!(fields[0], "ready");
    assert!(fields[1].starts_with("cr2se:"));
    let address: std::net::SocketAddr = fields[2].parse().unwrap();
    assert_ne!(address.port(), 0);
    assert!(std::net::TcpListener::bind(address).is_err());
    server.signal(rustix::process::Signal::TERM).unwrap();
    assert_eq!(server.line().unwrap().trim(), "stopped");
    assert!(server.finish().unwrap().success());
    assert!(std::net::TcpListener::bind(address).is_ok());
    let invalid = runtime::ProcessFixture::spawn(&["--unknown"]);
    assert!(invalid.error().is_ok());
}
