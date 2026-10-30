//! Durable contact snapshot failures preserve complete expected-ID associations.
use super::{
    FileStore,
    atomic::Cut,
    tests::{Fixture, limits},
};
use crate::modules::identity::{
    api::{Contact, IdentityId},
    authority::Authority,
    error::IdentityError,
};

// Atomic publication retains an old or new complete snapshot at every injected durability boundary.
#[test]
fn contact_snapshots_are_atomic_and_authorized() {
    let root = Fixture::new();
    let (mut store, admin) = FileStore::open(&root.0, limits()).unwrap();
    let first = Contact {
        expected_id: IdentityId([1; 32]),
        endpoint: None,
    };
    let next = Contact {
        endpoint: Some("127.0.0.1:42".parse().unwrap()),
        ..first
    };
    assert!(store.read_contacts(&admin).is_err());
    store.replace_contacts(&admin, &[first]).unwrap();
    let old = store.read_contacts(&admin).unwrap();
    let other = Authority::new().administration(4096).unwrap();
    assert_eq!(
        store.read_contacts(&other),
        Err(IdentityError::AccessDenied)
    );
    assert_eq!(
        store.replace_contacts(&other, &[]),
        Err(IdentityError::AccessDenied)
    );
    assert_eq!(
        store.replace_contacts(&admin, &[first, first]),
        Err(IdentityError::Conflict)
    );
    assert!(matches!(
        FileStore::open(&root.0, limits()),
        Err(IdentityError::Conflict)
    ));
    for cut in [Cut::Created, Cut::Written, Cut::Synced, Cut::Installed] {
        store.fault = None;
        store.replace_contacts(&admin, &[first]).unwrap();
        store.fault = Some(cut);
        assert_eq!(
            store.replace_contacts(&admin, &[next]),
            Err(if cut == Cut::Installed {
                IdentityError::OutcomeUnknown
            } else {
                IdentityError::Unavailable
            })
        );
        store.fault = None;
        assert_eq!(
            store.reconcile_contacts(&admin).unwrap(),
            vec![if cut == Cut::Installed { next } else { first }]
        );
        assert_eq!(old, vec![first]);
    }
    store.replace_contacts(&admin, &[]).unwrap();
    assert!(store.read_contacts(&admin).unwrap().is_empty());
    assert!(!root.0.join("identity.bin").exists());
}

// Local-format corruption, noncanonical spelling, duplicate IDs and configured bounds fail closed.
#[test]
fn corrupt_contact_records_are_rejected() {
    let root = Fixture::new();
    let (mut store, admin) = FileStore::open(&root.0, limits()).unwrap();
    let contact = Contact {
        expected_id: IdentityId([0; 32]),
        endpoint: None,
    };
    store.replace_contacts(&admin, &[contact]).unwrap();
    let line = crate::modules::identity::contact::format(contact).unwrap();
    for text in [
        "wrong".into(),
        "TSCON001\n\n".into(),
        format!("TSCON001\n{line}"),
        format!("TSCON001\n{line}\n{line}\n"),
        format!("TSCON001\n{}\n", line.to_lowercase()),
        "TSCON001\n\u{fffd}\n".into(),
        format!("TSCON001\n{}", format!("{line}\n").repeat(5)),
    ] {
        std::fs::write(root.0.join("contacts.bin"), text).unwrap();
        assert!(store.read_contacts(&admin).is_err());
    }
    let (mut small, admin) = FileStore::open(
        &root.0.join("small"),
        crate::modules::identity::config::Limits::new(256, 4, 1024, 2048, 9).unwrap(),
    )
    .unwrap();
    assert_eq!(
        small.replace_contacts(&admin, &[contact]),
        Err(IdentityError::ResourceLimit)
    );
    small.replace_contacts(&admin, &[]).unwrap();
    assert!(small.read_contacts(&admin).unwrap().is_empty());
}
