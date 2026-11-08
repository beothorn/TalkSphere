//! Atomic Services journal uncertainty and immutable image ownership.
use super::{
    Error,
    authority::{Action, Authority},
    contract::{Context, Limits},
    journal::Journal,
    persistence::{Memory, Persistence, Record},
};
use crate::modules::identity::api::IdentityId;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
struct Cut {
    record: Option<Record>,
    after: bool,
}
impl Persistence for Cut {
    // Restart reads only a complete last installed record.
    fn load(&mut self) -> Result<Option<Record>, Error> {
        Ok(self.record.clone())
    }
    // Lose one acknowledgement after storing new bytes; subsequent reload must resolve it.
    fn commit(&mut self, r: &Record) -> Result<(), Error> {
        self.record = Some(r.clone());
        if self.after {
            self.after = false;
            Err(Error::Unknown)
        } else {
            Ok(())
        }
    }
}
// Compare-and-replace never overwrites a concurrent image and return copies do not alias mutable state.
#[test]
fn image_compare_and_ownership() {
    let local = IdentityId([2; 32]);
    let a = Arc::new(Authority::new());
    let rg = a.grant(local, Action::Register, 1024).unwrap();
    let cg = a.grant(local, Action::Coordinate, 1024).unwrap();
    let recover = a.grant(local, Action::Recover, 1024).unwrap();
    let c = Context {
        version: 1,
        grant: &rg,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    };
    let j = Journal::open(
        &c,
        local,
        a.clone(),
        Limits::default(),
        Memory::default(),
        Some(vec![1]),
    )
    .unwrap();
    let cc = Context { grant: &cg, ..c };
    let copy = j.read(&cc, Action::Coordinate).unwrap();
    j.replace(&cc, &[1], vec![2]).unwrap();
    assert_eq!(copy, [1]);
    assert_eq!(j.replace(&cc, &copy, vec![3]), Err(Error::Conflict));
    assert_eq!(j.read(&cc, Action::Register), Err(Error::Denied));
    assert_eq!(j.read(&cc, Action::Coordinate), Ok(vec![2]));
    let rc = Context {
        grant: &recover,
        ..c
    };
    assert_eq!(j.recover(&rc), Ok(vec![2]));
    assert!(matches!(
        Journal::open(&rc, local, a, Limits::default(), Memory::default(), None),
        Err(Error::Unavailable)
    ));
}
// Lost durable acknowledgement gates reads/writes until authorized reload; no mutation is replayed.
#[test]
fn unknown_gates_until_recovery() {
    let local = IdentityId([2; 32]);
    let a = Arc::new(Authority::new());
    let cg = a.grant(local, Action::Coordinate, 1024).unwrap();
    let rg = a.grant(local, Action::Recover, 1024).unwrap();
    let c = Context {
        version: 1,
        grant: &rg,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    };
    let record = Record {
        identity: local,
        payload: vec![1],
    };
    let j = Journal::open(
        &c,
        local,
        a.clone(),
        Limits::default(),
        Cut {
            record: Some(record),
            after: true,
        },
        None,
    )
    .unwrap();
    let cc = Context { grant: &cg, ..c };
    assert_eq!(j.replace(&cc, &[1], vec![2]), Err(Error::Unknown));
    assert_eq!(j.read(&cc, Action::Coordinate), Err(Error::Unknown));
    assert_eq!(j.replace(&cc, &[1], vec![3]), Err(Error::Unknown));
    assert_eq!(j.recover(&c), Ok(vec![2]));
    assert_eq!(j.read(&cc, Action::Coordinate), Ok(vec![2]));
    assert!(matches!(
        Journal::open(
            &c,
            IdentityId([3; 32]),
            a,
            Limits::default(),
            Memory(Some(Record {
                identity: local,
                payload: vec![1]
            })),
            None
        ),
        Err(Error::Denied)
    ));
}
