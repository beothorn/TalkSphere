//! Executable Board commands preserve format/publication distinction and public output discipline.
use std::process::Command;
// Execute each new spelling's help without touching files or initializing operational logging.
#[test]
fn board_help_and_syntax() {
    for topic in [
        "board-validate",
        "board-show",
        "--board-file",
        "--board-store",
        "--publisher",
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_talksphere"))
            .args(["help", topic])
            .env("RUST_LOG", "[invalid")
            .output()
            .unwrap();
        assert!(out.status.success());
        assert!(out.stderr.is_empty());
        assert!(String::from_utf8(out.stdout).unwrap().contains("Usage:"));
    }
    for args in [
        vec!["board-validate"],
        vec!["board-validate", "--board-file", ""],
        vec![
            "board-show",
            "--board-store",
            "unused",
            "--publisher",
            "bad",
        ],
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_talksphere"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2));
    }
}
// Valid metadata produces only an honest format result; malformed input fails without printing payloads.
#[test]
fn board_validation_is_offline_and_bounded() {
    let path =
        std::env::temp_dir().join(format!("talksphere-board-cli-{}.json", std::process::id()));
    std::fs::write(
        &path,
        br#"{"version":1,"providedServices":[],"wantedServices":[]}"#,
    )
    .unwrap();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_talksphere"))
            .arg("board-validate")
            .arg("--board-file")
            .arg(&path)
            .env("RUST_LOG", "off")
            .output()
            .unwrap()
    };
    let out = run();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("format valid"));
    assert!(text.contains("publishability not established"));
    std::fs::write(&path, b"SECRET_INVALID_DOCUMENT").unwrap();
    let out = run();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&out.stderr).contains("SECRET_INVALID_DOCUMENT"));
    std::fs::write(&path, vec![b' '; 1_048_577]).unwrap();
    assert_eq!(run().status.code(), Some(1));
    std::fs::remove_file(path).unwrap();
}

// Inspection validates exact publisher and record contents without replacing a corrupt store.
#[cfg(target_os = "linux")]
#[test]
fn protected_board_inspection_checks_publisher() {
    use std::{collections::BTreeMap, os::unix::fs::PermissionsExt};
    use talksphere::modules::{
        board::{
            config::Limits,
            linux::FileStore,
            persistence::{Persistence, Record},
        },
        identity::{api::IdentityId, text},
    };
    let path =
        std::env::temp_dir().join(format!("talksphere-board-cli-store-{}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let id = IdentityId([7; 32]);
    let raw = br#"{"version":1,"providedServices":[],"wantedServices":[]}"#;
    let mut store = FileStore::open(&path, Limits::default()).unwrap();
    store
        .commit(&Record {
            publisher: id,
            board: raw.to_vec(),
            definitions: BTreeMap::new(),
        })
        .unwrap();
    drop(store);
    let run = |publisher| {
        Command::new(env!("CARGO_BIN_EXE_talksphere"))
            .arg("board-show")
            .arg("--board-store")
            .arg(&path)
            .arg("--publisher")
            .arg(text::format(publisher))
            .output()
            .unwrap()
    };
    let out = run(id);
    assert!(out.status.success());
    assert_eq!(out.stdout, [raw.as_slice(), b"\n"].concat());
    assert_eq!(run(IdentityId([8; 32])).status.code(), Some(1));
    std::fs::write(path.join("board.bin"), b"corrupt").unwrap();
    assert_eq!(run(id).status.code(), Some(1));
    assert_eq!(std::fs::read(path.join("board.bin")).unwrap(), b"corrupt");
    std::fs::remove_dir_all(path).unwrap();
}
