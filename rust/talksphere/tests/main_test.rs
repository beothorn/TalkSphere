use std::net::{Ipv4Addr, TcpListener};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

const LOOPBACK_ADDRESS: Ipv4Addr = Ipv4Addr::LOCALHOST;
const OPERATING_SYSTEM_SELECTED_PORT: u16 = 0;
const SECOND_PEER_START_DELAY_MILLISECONDS: u64 = 100;

fn two_available_local_ports() -> (u16, u16) {
    tracing::trace!(
        ">two_available_local_ports(): asking the operating system for two distinct unused ports"
    );

    let first_tcp_listener =
        TcpListener::bind((LOOPBACK_ADDRESS, OPERATING_SYSTEM_SELECTED_PORT)).unwrap();
    let second_tcp_listener =
        TcpListener::bind((LOOPBACK_ADDRESS, OPERATING_SYSTEM_SELECTED_PORT)).unwrap();
    let first_available_port = first_tcp_listener.local_addr().unwrap().port();
    let second_available_port = second_tcp_listener.local_addr().unwrap().port();

    tracing::trace!("<two_available_local_ports(): returning the two unused local ports");
    (first_available_port, second_available_port)
}

#[test]
fn two_run_commands_exchange_framed_greetings() {
    tracing::trace!(">two_run_commands_exchange_framed_greetings(): starting two TalkSphere peers");

    let (first_client_port, second_client_port) = two_available_local_ports();
    let first_peer = Command::new(env!("CARGO_BIN_EXE_talksphere"))
        .arg("run")
        .arg(first_client_port.to_string())
        .arg(second_client_port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    thread::sleep(Duration::from_millis(SECOND_PEER_START_DELAY_MILLISECONDS));
    let second_peer_output = Command::new(env!("CARGO_BIN_EXE_talksphere"))
        .arg("run")
        .arg(second_client_port.to_string())
        .arg(first_client_port.to_string())
        .output()
        .unwrap();
    let first_peer_output = first_peer.wait_with_output().unwrap();
    let first_peer_logs = format!(
        "{}{}",
        String::from_utf8_lossy(&first_peer_output.stdout),
        String::from_utf8_lossy(&first_peer_output.stderr)
    );
    let second_peer_logs = format!(
        "{}{}",
        String::from_utf8_lossy(&second_peer_output.stdout),
        String::from_utf8_lossy(&second_peer_output.stderr)
    );

    assert!(first_peer_output.status.success(), "{first_peer_logs}");
    assert!(second_peer_output.status.success(), "{second_peer_logs}");
    assert!(first_peer_logs.contains("hello world"), "{first_peer_logs}");
    assert!(
        second_peer_logs.contains("hello world"),
        "{second_peer_logs}"
    );

    tracing::trace!(
        "<two_run_commands_exchange_framed_greetings(): both TalkSphere peers received a greeting"
    );
}

#[test]
fn run_rejects_a_port_outside_the_unsigned_16_bit_range() {
    tracing::trace!(
        ">run_rejects_a_port_outside_the_unsigned_16_bit_range(): passing an invalid port to clap"
    );

    let process_output = Command::new(env!("CARGO_BIN_EXE_talksphere"))
        .args(["run", "70000", "8000"])
        .output()
        .unwrap();
    let error_output = String::from_utf8_lossy(&process_output.stderr);

    assert!(!process_output.status.success());
    assert!(error_output.contains("invalid value"), "{error_output}");

    tracing::trace!(
        "<run_rejects_a_port_outside_the_unsigned_16_bit_range(): clap rejected the invalid port"
    );
}

#[test]
fn version_prints_the_cargo_package_version() {
    tracing::trace!(">version_prints_the_cargo_package_version(): executing the version command");

    let process_output = Command::new(env!("CARGO_BIN_EXE_talksphere"))
        .arg("version")
        .output()
        .unwrap();
    let standard_output = String::from_utf8_lossy(&process_output.stdout);

    assert!(process_output.status.success());
    assert!(standard_output.contains(env!("CARGO_PKG_VERSION")));

    tracing::trace!("<version_prints_the_cargo_package_version(): the package version was printed");
}
