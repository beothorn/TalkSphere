//! Receiver authority and recovery isolation tested through typed public operations.
use super::*;
use crate::modules::ledger::{
    authority::{Action, Authority, Grant},
    contract::{Context, Limits, OperationKey},
    persistence::{Memory, Persistence},
    state::Ledger,
    terms::{Policy, Terms},
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
// Each action gets an explicit local caller scope; no peer data can make this grant.
fn context(g: &Grant) -> Context<'_> {
    Context {
        version: 1,
        grant: g,
        deadline: Instant::now() + Duration::from_secs(30),
        cancelled: false,
    }
}
// Forged tokens, foreign domains, cancellation and contract mismatch preserve original durable holds.
#[test]
fn forged_contexts_and_tokens() {
    let [a, b, _, _] = test_support::identities();
    let auth = Arc::new(Authority::new());
    let admin = auth.grant(a, Action::Administer, 65536).unwrap();
    let coord = auth.grant(a, Action::Coordinate, 65536).unwrap();
    let foreign = Authority::new()
        .grant(a, Action::Coordinate, 65536)
        .unwrap();
    let limits = Limits {
        accounts: 8,
        operations: 8,
        transfers: 8,
        claims: 8,
        quotes: 8,
        state_bytes: 65536,
    };
    let ledger = Ledger::initialize(
        a,
        auth.clone(),
        Memory::default(),
        limits,
        &context(&admin),
        100,
        100,
    )
    .unwrap();
    ledger.set_owned(&context(&admin), b, 100).unwrap();
    let key = OperationKey {
        requester: a,
        connection: [1; 32],
        stream: 1,
    };
    let terms = Terms {
        requester: a,
        provider: b,
        issuer: b,
        maximum: 7,
        fixed: true,
        accepted: vec![1],
        transfer: None,
        fee: None,
        spending_bound: 7,
    };
    assert!(
        ledger
            .reserve(&context(&foreign), key, terms.clone(), Policy::default())
            .is_err()
    );
    let reservation = ledger
        .reserve(&context(&coord), key, terms, Policy::default())
        .unwrap();
    let prepared = ledger
        .prepare(&context(&coord), &reservation, 7, vec![2])
        .unwrap();
    let mut forged = prepared.clone();
    forged.digest[0] ^= 1;
    assert_eq!(ledger.commit(&context(&coord), &forged), Err(Error::Denied));
    let mut call = context(&coord);
    call.version = 2;
    assert_eq!(ledger.commit(&call, &prepared), Err(Error::Unsupported));
    call.version = 1;
    call.cancelled = true;
    assert_eq!(ledger.commit(&call, &prepared), Err(Error::Cancelled));
    let image = ledger.inner.lock().unwrap().persistence.load().unwrap();
    auth.revoke();
    assert_eq!(
        ledger.commit(&context(&coord), &prepared),
        Err(Error::Denied)
    );
    assert_eq!(
        ledger.inner.lock().unwrap().persistence.load().unwrap(),
        image
    );
    let new_authority = Arc::new(Authority::new());
    let read = new_authority.grant(a, Action::Read, 65536).unwrap();
    let recovered = Ledger::open(a, new_authority, Memory(image), limits, &context(&read)).unwrap();
    assert_eq!(recovered.available(&context(&read), b, true), Ok(93));
}
// Dependency categories survive adapters instead of fabricating proof denial or storage rollback.
#[test]
fn error_categories() {
    use crate::modules::{
        cryptography::error::CryptoError as C, identity::error::IdentityError as I,
    };
    for (e, expected) in [
        (C::InvalidInput, Error::Invalid),
        (C::AuthenticationFailed, Error::Denied),
        (C::AccessDenied, Error::Denied),
        (C::Unavailable, Error::Unavailable),
        (C::ResourceLimit, Error::Limit),
    ] {
        assert_eq!(Error::from(e), expected)
    }
    for (e, expected) in [
        (I::InvalidInput, Error::Invalid),
        (I::Unsupported, Error::Unsupported),
        (I::IdentityMismatch, Error::Denied),
        (I::InvalidProof, Error::Denied),
        (I::AccessDenied, Error::Denied),
        (I::Unavailable, Error::Unavailable),
        (I::ResourceLimit, Error::Limit),
        (I::Conflict, Error::Conflict),
        (I::OutcomeUnknown, Error::Unknown),
    ] {
        assert_eq!(Error::from(e), expected)
    }
}
