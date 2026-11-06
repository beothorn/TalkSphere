//! Durable administration and recovery through public contracts.
use super::super::{authority::Grant, persistence::Memory};
use super::*;
use std::time::{Duration, Instant};
// Fresh positive bounded capacities used by all independent mutation scenarios.
fn limits() -> Limits {
    Limits {
        accounts: 16,
        operations: 32,
        transfers: 16,
        claims: 16,
        quotes: 8,
        state_bytes: 65536,
    }
}
// Borrow one explicit caller capability for a bounded test operation.
fn context(g: &Grant) -> Context<'_> {
    Context {
        version: 1,
        grant: g,
        deadline: Instant::now() + Duration::from_secs(30),
        cancelled: false,
    }
}
// Finite grants, exact local reconciliation and trust survive complete-image reopen.
#[test]
fn administration_and_reopen() {
    let [a, b, c, _] = super::super::test_support::identities();
    let auth = Arc::new(Authority::new());
    let admin = auth.grant(a, Action::Administer, 65536).unwrap();
    let read = auth.grant(a, Action::Read, 65536).unwrap();
    let ledger = Ledger::initialize(
        a,
        auth.clone(),
        Memory::default(),
        limits(),
        &context(&admin),
        10,
        20,
    )
    .unwrap();
    assert_eq!(ledger.grant(&context(&read), b, 1), Err(Error::Denied));
    ledger.grant(&context(&admin), b, 7).unwrap();
    assert_eq!(
        ledger.grant(&context(&admin), c, 7),
        Err(Error::Insufficient)
    );
    ledger.set_owned(&context(&admin), c, u64::MAX).unwrap();
    ledger
        .set_trust(&context(&admin), b, Trust::new(1).unwrap())
        .unwrap();
    assert_eq!(ledger.budgets(&context(&read)), Ok((3, 20)));
    assert_eq!(ledger.view(&context(&read), b).unwrap().issued, 7);
    assert_eq!(ledger.view(&context(&read), c).unwrap().owned, u64::MAX);
    assert_eq!(ledger.grant(&context(&admin), a, 1), Err(Error::Invalid));
    assert_eq!(ledger.grant(&context(&admin), b, 0), Err(Error::Invalid));
    assert_eq!(
        ledger.revoke(&context(&admin), b, 8),
        Err(Error::Insufficient)
    );
    ledger.revoke(&context(&admin), b, 2).unwrap();
    let image = ledger.inner.lock().unwrap().persistence.load().unwrap();
    let reopened = Ledger::open(
        a,
        auth.clone(),
        Memory(image.clone()),
        limits(),
        &context(&read),
    )
    .unwrap();
    assert_eq!(reopened.view(&context(&read), b).unwrap().issued, 5);
    assert_eq!(reopened.budgets(&context(&read)), Ok((3, 20)));
    assert!(matches!(
        Ledger::initialize(
            a,
            auth.clone(),
            Memory(image),
            limits(),
            &context(&admin),
            10,
            20
        ),
        Err(Error::Conflict)
    ));
    assert!(matches!(
        Ledger::open(
            a,
            auth.clone(),
            Memory::default(),
            limits(),
            &context(&read)
        ),
        Err(Error::Storage)
    ));
}
// Canonical complete-image validation rejects wrong identity, malformed schema and self-balances.
#[test]
fn recovered_state_validation() {
    let a = IdentityId([1; 32]);
    let mut s = State::default();
    let r = s.record(a, limits()).unwrap();
    assert!(State::load(&r, IdentityId([2; 32]), limits()).is_err());
    let mut bad = r.clone();
    bad.payload = b"{}".to_vec();
    assert!(State::load(&bad, a, limits()).is_err());
    bad.payload = r.payload.clone();
    bad.payload.push(b' ');
    assert!(State::load(&bad, a, limits()).is_err());
    s.accounts.owned.insert(a, 1);
    assert_eq!(s.record(a, limits()), Err(Error::Invalid));
}
// Both known failure and committed uncertainty preserve safety, while recovery only reloads.
#[test]
fn unknown_gate_and_recovery() {
    struct Fault {
        memory: Memory,
        cut: u8,
    }
    impl Persistence for Fault {
        // Return acknowledged or possibly committed complete image.
        fn load(&mut self) -> Result<Option<Record>, Error> {
            self.memory.load()
        }
        // Inject failure exactly before or after image replacement.
        fn commit(&mut self, r: &Record) -> Result<(), Error> {
            if self.cut == 1 {
                return Err(Error::Storage);
            }
            self.memory.commit(r)?;
            if self.cut == 2 {
                return Err(Error::Unknown);
            }
            Ok(())
        }
    }
    let [a, b, _, _] = super::super::test_support::identities();
    let auth = Arc::new(Authority::new());
    let admin = auth.grant(a, Action::Administer, 65536).unwrap();
    let read = auth.grant(a, Action::Read, 65536).unwrap();
    let coord = auth.grant(a, Action::Coordinate, 65536).unwrap();
    let ledger = Ledger::initialize(
        a,
        auth.clone(),
        Fault {
            memory: Memory::default(),
            cut: 0,
        },
        limits(),
        &context(&admin),
        10,
        10,
    )
    .unwrap();
    ledger.inner.lock().unwrap().persistence.cut = 1;
    assert_eq!(ledger.grant(&context(&admin), b, 7), Err(Error::Storage));
    assert_eq!(ledger.view(&context(&read), b).unwrap().issued, 0);
    ledger.inner.lock().unwrap().persistence.cut = 2;
    assert_eq!(ledger.grant(&context(&admin), b, 7), Err(Error::Unknown));
    assert_eq!(ledger.view(&context(&read), b), Err(Error::Unknown));
    assert_eq!(ledger.grant(&context(&admin), b, 1), Err(Error::Unknown));
    ledger.recover(&context(&coord)).unwrap();
    assert_eq!(ledger.view(&context(&read), b).unwrap().issued, 7);
}
