//! Exact fee negotiation tested with real owner signatures.
use super::*;
use crate::modules::cryptography::{
    credential,
    primitives::{keys::signing_key, signature},
    test_support::ScriptedEntropy,
};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
// A valid fresh order, signature and independent local issuer authority enable each failure mutation.
#[test]
fn exact_stream_and_fee() {
    let mut entropy = ScriptedEntropy(VecDeque::from([Ok(vec![1; 32])]));
    let key = signing_key(&mut entropy).unwrap();
    let owner = IdentityId(credential::identity_id(&key.public_key).unwrap());
    let issuer = IdentityId([3; 32]);
    let order = Order {
        transfer_id: [9; 16],
        issuer,
        owner,
        new_owner: IdentityId([2; 32]),
        amount: 9,
    };
    let signature = signature::sign(&key.private_key, &order.encode().unwrap(), 121).unwrap();
    let auth = Arc::new(Authority::new());
    let control = auth.grant(issuer, Action::Control, 65536).unwrap();
    let context = Context {
        version: 1,
        grant: &control,
        deadline: Instant::now() + Duration::from_secs(30),
        cancelled: false,
    };
    let gate = Quotes::new(issuer, auth.clone(), 1).unwrap();
    assert!(Quotes::new(issuer, auth.clone(), 0).is_err());
    let peer = Peer {
        owner,
        connection: [1; 32],
        stream: 1,
    };
    let request = || Request {
        order: order.clone(),
        key: key.public_key,
        signature,
        maximum_fee: 2,
        fee: 2,
        expires: 20,
    };
    let mut bad = request();
    bad.fee = 9;
    assert!(gate.quote(&context, peer, bad, 10).is_err());
    let mut bad = request();
    bad.maximum_fee = 1;
    assert!(gate.quote(&context, peer, bad, 10).is_err());
    let q = gate.quote(&context, peer, request(), 10).unwrap();
    assert_eq!(q.fee(), 2);
    assert!(gate.quote(&context, peer, request(), 10).is_err());
    assert!(
        gate.quote(&context, Peer { stream: 2, ..peer }, request(), 10)
            .is_err()
    );
    assert!(
        gate.accept(&context, &q, Peer { stream: 2, ..peer }, 2, 10)
            .is_err()
    );
    assert!(gate.accept(&context, &q, peer, 1, 10).is_err());
    assert!(gate.accept(&context, &q, peer, 2, 20).is_err());
    let accepted = gate.accept(&context, &q, peer, 2, 10).unwrap();
    assert_eq!(accepted.order(), &order);
    assert_eq!(accepted.fee(), 2);
    accepted.check_receiver(&auth, issuer).unwrap();
    assert!(
        accepted
            .check_receiver(&Arc::new(Authority::new()), issuer)
            .is_err()
    );
    assert!(gate.accept(&context, &q, peer, 2, 10).is_err());
    gate.discard(&context, peer).unwrap();
}
