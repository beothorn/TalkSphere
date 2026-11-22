//! Node administration is explicit, never replaces existing state and reports failures on stderr.
#![cfg(target_os = "linux")]
use std::process::{Command, Output};
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_talksphere"))
        .args(args)
        .output()
        .unwrap()
}
// Initialization prints only the public ID, inspection reports the state, repeated initialization
// fails with status 1 leaving the node unchanged, and syntax errors exit with status 2.
#[test]
fn node_init_and_show_are_explicit_and_safe() {
    let root = std::env::temp_dir().join(format!("talksphere-node-cli-{}", std::process::id()));
    let path = root.to_str().unwrap();
    let missing = run(&["node-show", "--node-root", path]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(!root.exists());
    let created = run(&["node-init", "--node-root", path]);
    assert!(created.status.success());
    let id = String::from_utf8(created.stdout).unwrap();
    assert!(id.trim().starts_with("cr2se:") && id.trim().lines().count() == 1);
    let shown = run(&["node-show", "--node-root", path]);
    let text = String::from_utf8(shown.stdout.clone()).unwrap();
    assert!(
        shown.status.success() && text.contains(id.trim()) && text.contains("state: initialized")
    );
    let before = std::fs::read(root.join("node.json")).unwrap();
    let again = run(&["node-init", "--node-root", path]);
    assert_eq!(again.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&again.stderr).contains("already contains state"));
    assert_eq!(std::fs::read(root.join("node.json")).unwrap(), before);
    assert_eq!(run(&["node-init"]).status.code(), Some(2));
    std::fs::remove_dir_all(root).unwrap();
}
// An invalid configuration file fails before the root is created; a requester-only file creates no Storage.
#[test]
fn node_init_validates_configuration_before_effects() {
    let dir = std::env::temp_dir().join(format!("talksphere-node-cfg-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let bad = dir.join("bad.json");
    std::fs::write(&bad, br#"{"version":1,"capacity":{"bytes":0}}"#).unwrap();
    let root = dir.join("node");
    let out = run(&[
        "node-init",
        "--node-root",
        root.to_str().unwrap(),
        "--config",
        bad.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(1));
    assert!(!root.exists());
    let good = dir.join("good.json");
    std::fs::write(&good, br#"{"version":1,"offering":null}"#).unwrap();
    let out = run(&[
        "node-init",
        "--node-root",
        root.to_str().unwrap(),
        "--config",
        good.to_str().unwrap(),
    ]);
    assert!(out.status.success());
    assert!(!root.join("storage").exists());
    std::fs::remove_dir_all(dir).unwrap();
}
// Detailed help documents both commands and their options.
#[test]
fn node_help_covers_commands_and_options() {
    for topic in ["node-init", "node-show", "--node-root", "--config"] {
        let out = run(&["help", topic]);
        assert!(out.status.success());
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(text.contains("node-init") && text.contains("--node-root"));
    }
    assert!(
        String::from_utf8(run(&["--help"]).stdout)
            .unwrap()
            .contains("node-init | node-show")
    );
}
// An absent policy file reports defaults, a valid one is accepted, and an invalid one is reported by
// node-show and stops `serve --node-root` with status 1 before the node listens.
#[test]
fn policy_file_is_validated_at_inspection_and_startup() {
    let dir = std::env::temp_dir().join(format!("talksphere-node-policy-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let root = dir.join("node");
    let path = root.to_str().unwrap();
    assert!(run(&["node-init", "--node-root", path]).status.success());
    let shown = |expect: &str| {
        let out = run(&["node-show", "--node-root", path]);
        assert!(
            String::from_utf8(out.stdout).unwrap().contains(expect),
            "{expect}"
        );
    };
    shown("policy: defaults (no policy.json)");
    std::fs::write(
        root.join("policy.json"),
        br#"{"version":1,"redundancy":{"target":10}}"#,
    )
    .unwrap();
    shown("policy: policy.json valid");
    std::fs::write(
        root.join("policy.json"),
        br#"{"version":1,"quota":{"pending":0}}"#,
    )
    .unwrap();
    shown("policy: policy.json invalid");
    let served = run(&["serve", "--node-root", path, "--listen", "127.0.0.1:0"]);
    assert_eq!(served.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&served.stderr);
    assert!(
        stderr.contains("quota.pending") && !stderr.contains("ready"),
        "{stderr}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
