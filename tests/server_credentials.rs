//! Protected startup checks retain key authority and immutable contacts before network composition.
#![cfg(target_os = "linux")]
#[path = "support/runtime.rs"]
mod runtime;
use std::process::Command;
use talksphere::{
    composition::server::{
        config::{Config, Options},
        credentials,
    },
    modules::{
        identity::{
            api::{Contact, IdentityId},
            config::Limits,
            storage::FileStore,
        },
        transport::ports::identity::Identity,
    },
};
// Missing/corrupt keys fail without creation; successful loading owns the writer lock and copied contact snapshot.
#[test]
fn server_credentials_are_existing_scoped_and_exclusive() {
    let fixture = runtime::ProcessFixture::spawn(&["--version"]);
    assert!(fixture.line().is_ok());
    let path = fixture.root.join("identity");
    let config = Config::new(Options::local(path.clone()).unwrap()).unwrap();
    assert!(credentials::load(&config, false).is_err());
    assert!(!path.exists());
    assert!(
        Command::new(env!("CARGO_BIN_EXE_talksphere"))
            .args(["identity-init", "--identity-store"])
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    let loaded = credentials::load(&config, false).unwrap();
    assert_eq!(
        loaded.identity.public_identity().unwrap().id,
        loaded.local_id
    );
    assert!(credentials::load(&config, false).is_err());
    let mut transport = talksphere::composition::server::transport::compose(
        Config::new(Options::local(path.clone()).unwrap()).unwrap(),
        loaded,
    )
    .unwrap();
    assert_eq!(transport.driver.resources().connections(), 0);
    assert_eq!(transport.driver.resources().outbound_bytes(), 0);
    let occupied = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut conflict = Options::local(path.clone()).unwrap();
    conflict.listen = occupied.local_addr().unwrap();
    transport.config = Config::new(conflict).unwrap();
    assert!(matches!(
        transport.listen(),
        Err(talksphere::composition::server::Failure::Network)
    ));
    drop(occupied);
    transport.config = Config::new(Options::local(path.clone()).unwrap()).unwrap();
    let ready = transport.listen().unwrap();
    assert!(ready.address.ip().is_loopback());
    assert_ne!(ready.address.port(), 0);
    assert!(std::net::TcpListener::bind(ready.address).is_err());
    assert!(transport.listen().is_err());
    assert!(credentials::load(&config, false).is_err());
    drop(transport);
    let rebound = std::net::TcpListener::bind(ready.address).unwrap();
    drop(rebound);
    assert!(credentials::load(&config, true).is_err());
    let limits = Limits::new(1024, 64, 65548, 65536, 65536).unwrap();
    let (mut store, admin) = FileStore::open(&path, limits).unwrap();
    let contact = Contact {
        expected_id: IdentityId([1; 32]),
        endpoint: Some("127.0.0.1:42".parse().unwrap()),
    };
    store.replace_contacts(&admin, &[contact]).unwrap();
    drop(store);
    let loaded = credentials::load(&config, true).unwrap();
    assert_eq!(loaded.contacts, vec![contact]);
    let mut node = talksphere::composition::server::transport::compose(
        Config::new(Options::local(path.clone()).unwrap()).unwrap(),
        loaded,
    )
    .unwrap();
    assert!(node.connect_contacts().is_err());
    node.listen().unwrap();
    let attempts = node.connect_contacts().unwrap();
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].expected_id.0, contact.expected_id.0);
    assert_eq!(
        node.connection_count(),
        usize::from(attempts[0].admitted.is_ok())
    );
    assert!(node.connect_contacts().is_err());
    node.begin_shutdown(std::time::Instant::now()).unwrap();
    node.begin_shutdown(std::time::Instant::now()).unwrap();
    assert!(
        node.shutdown_progress(
            std::time::Instant::now() + std::time::Duration::from_secs(6),
            false
        )
        .is_err()
    );
    drop(node);
    std::fs::write(path.join("identity.bin"), b"corrupt").unwrap();
    assert!(credentials::load(&config, false).is_err());
    assert_eq!(
        std::fs::read(path.join("identity.bin")).unwrap(),
        b"corrupt"
    );
    // Exercise independent stderr capture too, keeping the shared fixture contract checked in this suite.
    let invalid = runtime::ProcessFixture::spawn(&["--unknown"]);
    assert!(invalid.error().is_ok());
}
