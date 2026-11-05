//! Explicit CLI administration preserves protected identity authority across separate invocations.
#![cfg(target_os = "linux")]
use std::{fs, process::Command};
// Initialization is explicit, duplicate initialization preserves keys, and inspection does not replace missing/corrupt state.
#[test]
fn identity_commands_preserve_existing_authority() {
    let path = std::env::temp_dir().join(format!("talksphere-admin-{}", std::process::id()));
    let run = |command: &str| {
        Command::new(env!("CARGO_BIN_EXE_talksphere"))
            .args([command, "--identity-store"])
            .arg(&path)
            .output()
            .unwrap()
    };
    assert!(!run("identity-show").status.success());
    assert!(!path.exists());
    let created = run("identity-init");
    assert!(created.status.success());
    assert!(created.stdout.starts_with(b"cr2se:"));
    let bytes = zeroize::Zeroizing::new(fs::read(path.join("identity.bin")).unwrap());
    assert!(!run("identity-init").status.success());
    assert_eq!(run("identity-show").stdout, created.stdout);
    assert_eq!(
        &*zeroize::Zeroizing::new(fs::read(path.join("identity.bin")).unwrap()),
        &*bytes
    );
    fs::write(path.join("identity.bin"), b"corrupt").unwrap();
    assert!(!run("identity-show").status.success());
    assert_eq!(fs::read(path.join("identity.bin")).unwrap(), b"corrupt");
    fs::remove_dir_all(path).unwrap();
}
