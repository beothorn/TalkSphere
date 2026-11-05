//! Executable logging configuration keeps help available and operational failures on stderr.
use std::process::Command;
// Filtering changes operational traces without contaminating stdout or making help depend on a valid filter.
#[test]
fn logging_filters_and_output_streams() {
    let run = |filter: &str, args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_talksphere"))
            .env("RUST_LOG", filter)
            .args(args)
            .output()
            .unwrap()
    };
    let invalid = run(
        "talksphere=not_a_level",
        &["serve", "--identity-store", "missing-logging-fixture"],
    );
    assert_eq!(invalid.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("invalid RUST_LOG"));
    assert!(invalid.stdout.is_empty());
    assert!(run("bad=not_a_level", &["--help"]).status.success());
    let trace = run(
        "talksphere=trace",
        &["serve", "--identity-store", "missing-logging-fixture"],
    );
    assert!(String::from_utf8_lossy(&trace.stderr).contains("TRACE"));
    assert!(trace.stdout.is_empty());
    let off = run(
        "off",
        &["serve", "--identity-store", "missing-logging-fixture"],
    );
    assert!(!String::from_utf8_lossy(&off.stderr).contains("TRACE"));
}
