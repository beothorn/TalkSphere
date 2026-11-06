//! Commit uncertainty tests observe the public owner and persistence contracts.
use super::{
    Error,
    authority::{Action, Authority},
    config::Limits,
    contract::Context,
    persistence::{Persistence, Record},
    prepared::Prepared,
    semantics::Unavailable,
    state::Provider,
};
use crate::modules::identity::api::IdentityId;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
struct Script {
    data: Arc<Mutex<Data>>,
}
#[derive(Default)]
struct Data {
    record: Option<Record>,
    cuts: VecDeque<Result<(), Error>>,
    writes: usize,
}
impl Persistence for Script {
    // Load reports only the actual committed record; it never invents rollback after a lost reply.
    fn load(&mut self) -> Result<Option<Record>, Error> {
        Ok(self.data.lock().unwrap().record.clone())
    }
    // An Unknown response may follow commitment; a known Storage failure preserves old durable bytes.
    fn commit(&mut self, r: &Record) -> Result<(), Error> {
        let mut data = self.data.lock().unwrap();
        data.writes += 1;
        let result = data.cuts.pop_front().unwrap();
        if result != Err(Error::Storage) {
            data.record = Some(r.clone());
        }
        result
    }
}
// A lost acknowledgement gates reads/new writes until recovery, without another commit or changed old pins.
#[test]
fn uncertain_commit_requires_recovery_without_replay() {
    let id = IdentityId([1; 32]);
    let a = Arc::new(Authority::new());
    let l = Limits::default();
    let data = Arc::new(Mutex::new(Data {
        cuts: VecDeque::from([Ok(()), Err(Error::Storage), Err(Error::Unknown)]),
        ..Data::default()
    }));
    let p = Provider::open(
        id,
        a.clone(),
        l,
        Arc::new(Unavailable),
        Box::new(Script { data: data.clone() }),
    )
    .unwrap();
    let grant = a.grant(id, Action::Publish, l.publication_bytes).unwrap();
    let c = Context {
        grant: &grant,
        deadline: Instant::now() + Duration::from_secs(10),
        cancelled: false,
    };
    let read = a.grant(id, Action::Read, l.publication_bytes).unwrap();
    let rc = Context { grant: &read, ..c };
    assert!(matches!(p.read(&rc), Err(Error::Missing)));
    let raw = br#"{"version":1,"providedServices":[],"wantedServices":[]}"#;
    let make = |marker: &str| {
        let mut bytes = raw.to_vec();
        bytes.extend_from_slice(marker.as_bytes());
        Prepared::new(id, &bytes, &BTreeMap::new(), l, &Unavailable).unwrap()
    };
    let old = p.publish(&c, make("")).unwrap();
    assert!(matches!(p.publish(&c, make(" ")), Err(Error::Storage)));
    assert_eq!(p.read(&rc).unwrap().bytes(), old.bytes());
    assert!(matches!(p.publish(&c, make("  ")), Err(Error::Unknown)));
    assert!(matches!(p.read(&rc), Err(Error::Unknown)));
    assert!(matches!(p.publish(&c, make("   ")), Err(Error::Unknown)));
    assert_eq!(data.lock().unwrap().writes, 3);
    p.recover(&c).unwrap();
    assert_eq!(p.read(&rc).unwrap().bytes().len(), old.bytes().len() + 2);
    assert_eq!(data.lock().unwrap().writes, 3);
    assert_eq!(old.bytes(), raw);
    drop(p);
    let reopened =
        Provider::open(id, a, l, Arc::new(Unavailable), Box::new(Script { data })).unwrap();
    assert_eq!(reopened.read(&rc).unwrap().bytes().len(), raw.len() + 2);
}
