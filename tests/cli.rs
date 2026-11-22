use std::process::{Command, Output};

fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_talksphere"))
        .args(arguments)
        .output()
        .expect("TalkSphere executable should run")
}

fn successful_stdout(arguments: &[&str]) -> String {
    let output = run(arguments);
    assert!(output.status.success(), "{arguments:?}: {output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    String::from_utf8(output.stdout).expect("CLI output should be UTF-8")
}

#[test]
fn overview_is_successful_and_discovers_detailed_help() {
    let overview = successful_stdout(&["--help"]);
    assert!(overview.contains("talksphere help <topic>"));
    assert!(overview.contains("serve"));
    assert!(overview.contains("--version"));
    assert!(overview.contains(
        "authenticated Transport server available on Linux; Peer Protocol library and explicit custom-node example available; minimum useful paid Storage node via node-init, serve --node-root and node-ctl"
    ));
    for arguments in [&[][..], &["-h"], &["help"]] {
        assert_eq!(successful_stdout(arguments), overview);
    }
}

#[test]
fn command_and_option_details_are_accessible_without_starting_server() {
    for topic in [
        "help",
        "version",
        "serve",
        "--help",
        "-h",
        "--version",
        "-V",
    ] {
        let details = successful_stdout(&["help", topic]);
        assert!(details.contains("Usage:"), "{topic}: {details}");
    }
    let serve_help = successful_stdout(&["help", "serve"]);
    assert!(serve_help.contains("opens no sockets"));
    assert!(serve_help.contains("existing protected keys"));
    assert!(serve_help.contains("plans/4_Transport_integration/PLAN.md"));
    assert_eq!(successful_stdout(&["serve", "--help"]), serve_help);
    assert_eq!(successful_stdout(&["serve", "-h"]), serve_help);
    assert_eq!(
        successful_stdout(&["help", "--version"]),
        successful_stdout(&["help", "version"])
    );
}

// The peer policy topic is discoverable under each spelling, and the node-ctl help names every policy,
// wanted-Storage and replica action so none is undocumented.
#[test]
fn policy_help_is_discoverable_and_node_ctl_help_names_every_new_action() {
    let policy = successful_stdout(&["help", "policy"]);
    for name in ["policy.json", "policy-sim", "policy_sim"] {
        assert_eq!(successful_stdout(&["help", name]), policy, "{name}");
    }
    assert!(policy.contains("docs/policy/PARAMETERS.md") && policy.contains("policy_sim"));
    assert!(successful_stdout(&["--help"]).contains("help policy"));
    let node_ctl = successful_stdout(&["help", "node-ctl"]);
    for action in [
        "fulfil",
        "wanted-add",
        "replica-add",
        "replicas",
        "policy-show",
        "policy-explain",
    ] {
        assert!(node_ctl.contains(action), "node-ctl help lacks {action}");
    }
    let node = successful_stdout(&["help", "node-show"]);
    assert!(node.contains("policy.json"));
    let serve = successful_stdout(&["help", "--contact"]);
    assert!(serve.contains("reconnectMaxSeconds") && serve.contains("pingSeconds"));
}

#[test]
fn version_is_derived_from_package_metadata() {
    let expected = format!("talksphere {}\n", env!("CARGO_PKG_VERSION"));
    assert_eq!(successful_stdout(&["--version"]), expected);
    assert_eq!(successful_stdout(&["-V"]), expected);
}

// Missing required store selection is a syntax error and cannot start a server.
#[test]
fn serve_requires_explicit_identity_selection() {
    let output = run(&["serve"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("--identity-store"));
    assert!(error.contains("talksphere --help"));
}

#[test]
fn invalid_arguments_report_usage_errors_on_stderr() {
    for arguments in [
        vec!["unknown"],
        vec!["help", "unknown"],
        vec!["serve", "--listen", "127.0.0.1:8000"],
        vec!["--version", "extra"],
        vec!["--help", "extra"],
    ] {
        let output = run(&arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains("error:"));
        assert!(error.contains("talksphere --help"));
    }
}

#[cfg(unix)]
#[test]
fn non_utf8_arguments_report_a_usage_error_without_panicking() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    let output = Command::new(env!("CARGO_BIN_EXE_talksphere"))
        .arg(OsString::from_vec(vec![0xff]))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("valid UTF-8"));
    assert!(!error.contains("panicked"));
}

// Local Services exercise instructions remain discoverable without initializing stores or opening sockets.
#[test]
fn services_help_explains_scope_and_recovery() {
    let details = successful_stdout(&["help", "services"]);
    for text in [
        "cargo run --locked --example services",
        "price=7, remaining=93",
        "volatile",
        "fresh grants",
        "no application protocol",
    ] {
        assert!(details.contains(text), "{details}");
    }
}

// Peer Protocol remains discoverable without creating keys, stores, grants or network listeners.
#[test]
fn peer_protocol_help_explains_explicit_nodes_and_configuration() {
    let overview = successful_stdout(&["--help"]);
    assert!(overview.contains("help peer-protocol"));
    let details = successful_stdout(&["help", "peer-protocol"]);
    for text in [
        "cargo run --locked --example peer -- run",
        "help run",
        "volatile",
        "no application protocol",
        "bounded workers",
        "RUST_LOG",
        "RUNTIME.md",
        "status 0",
    ] {
        assert!(details.contains(text), "{details}");
    }
}

// The Storage topic advertises the explicit Linux exercise and exposes detailed help without acquiring state.
#[test]
fn storage_help_describes_the_native_exercise() {
    assert!(successful_stdout(&["--help"]).contains("help storage"));
    let details = successful_stdout(&["help", "storage"]);
    assert!(details.contains("cargo run --locked --example storage -- run"));
    assert!(details.contains("cargo run --locked --example storage -- help run"));
    assert!(details.contains("RUST_LOG"));
}
