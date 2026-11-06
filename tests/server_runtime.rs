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

// A real silent TCP peer receives no authenticated session and is closed by the configured handshake timer.
#[test]
fn silent_peer_is_closed_without_stopping_listener() {
    use std::io::Read;
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
    let mut server = runtime::ProcessFixture::spawn(&[
        "serve",
        "--identity-store",
        path.to_str().unwrap(),
        "--handshake-ms",
        "100",
        "--max-connections",
        "1",
    ]);
    let ready = server.line().unwrap();
    let address = ready.split_whitespace().nth(2).unwrap();
    let mut socket = std::net::TcpStream::connect(address).unwrap();
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    let mut hello = [0; 112];
    socket.read_exact(&mut hello).unwrap();
    assert_eq!(hello[1], 1);
    let mut tail = Vec::new();
    socket.take(2048).read_to_end(&mut tail).unwrap();
    assert_eq!(server.line().unwrap().trim(), "connection-closed");
    let again = std::net::TcpStream::connect(address).unwrap();
    drop(again);
    server.signal(rustix::process::Signal::TERM).unwrap();
    for _ in 0..8 {
        if server.line().unwrap().trim() == "stopped" {
            assert!(server.finish().unwrap().success());
            return;
        }
    }
    panic!("server did not stop after bounded silent-peer test");
}
