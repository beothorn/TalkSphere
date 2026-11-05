//! Fixture self-checks for later real server process tests.
#[path = "support/runtime.rs"]
mod runtime;

// A real child supplies bounded stdout, and dropping its fixture removes its isolated working directory.
#[test]
fn process_fixture_reads_output_and_cleans_owned_state() {
    let process = runtime::ProcessFixture::spawn(&["--version"]);
    let path = process.root.clone();
    assert!(process.line().unwrap().starts_with("talksphere "));
    drop(process);
    assert!(!path.exists());
}

// Invalid CLI input produces a captured stderr diagnostic independently of stdout.
#[test]
fn process_fixture_captures_errors() {
    let process = runtime::ProcessFixture::spawn(&["--unknown-fixture-option"]);
    assert!(process.error().unwrap().starts_with("error:"));
}
