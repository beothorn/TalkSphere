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
    assert!(overview.contains("Transport interface defined; networking is not implemented"));
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
    assert!(serve_help.contains("public interface is defined"));
    assert!(serve_help.contains("plans/1_Transport_module/PLAN.md"));
    assert_eq!(successful_stdout(&["serve", "--help"]), serve_help);
    assert_eq!(successful_stdout(&["serve", "-h"]), serve_help);
    assert_eq!(
        successful_stdout(&["help", "--version"]),
        successful_stdout(&["help", "version"])
    );
}

#[test]
fn version_is_derived_from_package_metadata() {
    let expected = format!("talksphere {}\n", env!("CARGO_PKG_VERSION"));
    assert_eq!(successful_stdout(&["--version"]), expected);
    assert_eq!(successful_stdout(&["-V"]), expected);
}

#[test]
fn serve_fails_explicitly_until_transport_is_implemented() {
    let output = run(&["serve"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("Transport networking is not implemented"));
    assert!(error.contains("no server was started"));
    assert!(error.contains("talksphere help serve"));
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
