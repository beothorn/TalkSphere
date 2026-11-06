//! A separate process reopens an acknowledged Linux Board store through public contracts.
#![cfg(target_os = "linux")]
use std::{collections::BTreeMap, os::unix::fs::PermissionsExt};
use talksphere::modules::{
    board::{
        config::Limits,
        linux::FileStore,
        persistence::{Persistence, Record},
    },
    identity::api::IdentityId,
};
// Child invocation verifies only persisted bytes, with no shared heap or retained descriptors.
#[test]
fn reopen_child() {
    let Some(path) = std::env::var_os("TALKSPHERE_BOARD_REOPEN") else {
        return;
    };
    let mut store = FileStore::open(std::path::Path::new(&path), Limits::default()).unwrap();
    let r = store.load().unwrap().unwrap();
    assert_eq!(r.publisher, IdentityId([7; 32]));
    assert_eq!(
        r.board,
        br#"{"version":1,"providedServices":[],"wantedServices":[]}"#
    );
}
// Acknowledged records survive descriptor release and a new process; the parent owns cleanup.
#[test]
fn acknowledged_snapshot_survives_process_restart() {
    let path =
        std::env::temp_dir().join(format!("talksphere-board-process-{}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut store = FileStore::open(&path, Limits::default()).unwrap();
    store
        .commit(&Record {
            publisher: IdentityId([7; 32]),
            board: br#"{"version":1,"providedServices":[],"wantedServices":[]}"#.to_vec(),
            definitions: BTreeMap::new(),
        })
        .unwrap();
    drop(store);
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "reopen_child"])
        .env("TALKSPHERE_BOARD_REOPEN", &path)
        .status()
        .unwrap();
    std::fs::remove_dir_all(path).unwrap();
    assert!(status.success());
}
