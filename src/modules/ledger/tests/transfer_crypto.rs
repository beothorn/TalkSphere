//! Real strict Ed25519 transfer authentication with deterministic public test keys.
use super::*;
use crate::modules::cryptography::{
    primitives::{keys::signing_key, signature},
    secrets::{Secret, Signing},
    test_support::ScriptedEntropy,
};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
struct Signer(Secret<Signing>);
impl TransferSigner for Signer {
    // Real primitive signing under guarded test authority, never an always-valid proof stub.
    fn sign(&self, bytes: &[u8; 121]) -> Result<[u8; 64], Error> {
        signature::sign(&self.0, bytes, 121).map_err(|_| Error::Denied)
    }
}
// Exact-byte signatures reject changed orders, callers, issuer and weak/malformed proofs.
#[test]
fn real_transfer_signatures() {
    let mut entropy = ScriptedEntropy(VecDeque::from([Ok(vec![1; 32])]));
    let key = signing_key(&mut entropy).unwrap();
    let local = IdentityId(credential::identity_id(&key.public_key).unwrap());
    let issuer = IdentityId([3; 32]);
    let new_owner = IdentityId([2; 32]);
    let auth = Authority::new();
    let grant = auth.grant(local, Action::Spend, 121).unwrap();
    let context = Context {
        version: 1,
        grant: &grant,
        deadline: Instant::now() + Duration::from_secs(30),
        cancelled: false,
    };
    let signer = Signer(key.private_key);
    let mut entropy = ScriptedEntropy(VecDeque::from([Ok(vec![9; 16])]));
    let (order, sig) = create(
        &context,
        &auth,
        local,
        Create {
            issuer,
            new_owner,
            amount: 9,
            public_key: key.public_key,
        },
        &mut entropy,
        &signer,
    )
    .unwrap();
    assert_eq!(order.transfer_id, [9; 16]);
    verify(local, issuer, &order, &key.public_key, &sig).unwrap();
    let mut changed = order.clone();
    changed.amount = 8;
    assert!(verify(local, issuer, &changed, &key.public_key, &sig).is_err());
    assert!(verify(new_owner, issuer, &order, &key.public_key, &sig).is_err());
    assert!(verify(local, new_owner, &order, &key.public_key, &sig).is_err());
    assert!(verify(local, issuer, &order, &[0; 32], &sig).is_err());
    assert!(verify(local, issuer, &order, &key.public_key, &sig[..63]).is_err());
    assert!(
        create(
            &context,
            &auth,
            local,
            Create {
                issuer,
                new_owner,
                amount: 9,
                public_key: key.public_key
            },
            &mut entropy,
            &signer
        )
        .is_err()
    );
    auth.revoke();
    assert!(
        create(
            &context,
            &auth,
            local,
            Create {
                issuer,
                new_owner,
                amount: 9,
                public_key: key.public_key
            },
            &mut entropy,
            &signer
        )
        .is_err()
    );
}
