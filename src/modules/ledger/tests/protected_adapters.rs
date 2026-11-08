//! Real protected Identity adapters preserve purpose and byte limits without extracting keys.
#![cfg(target_os = "linux")]
use super::*;
use crate::modules::{
    cryptography::{api::RecipientCredential, provider::Provider, random::SystemEntropy},
    identity::{
        api::IdentityProvider,
        authority::{Decrypt, Sign},
        config::Limits as IdentityLimits,
        provider::Identity,
        storage::FileStore,
    },
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
// Exercise both protected adapters with real owner-only Identity storage and real envelope/signature operations.
#[test]
fn protected_signing_and_decryption() {
    let path = std::env::temp_dir().join(format!(
        "talksphere-ledger-identity-adapters-{}",
        std::process::id()
    ));
    let limits = IdentityLimits::new(1024, 16, 4096, 4096, 4096).unwrap();
    let (store, admin) = FileStore::open(&path, limits).unwrap();
    let mut identity = Identity::new(store, SystemEntropy, limits, 1).unwrap();
    let (handle, public) = identity.create(&admin).unwrap();
    let sign = identity.grant::<Sign>(&admin, &handle, 121).unwrap();
    let short = identity.grant::<Sign>(&admin, &handle, 1).unwrap();
    let decrypt = identity.grant::<Decrypt>(&admin, &handle, 4096).unwrap();
    let local = public.identity.id();
    let issuer = crate::modules::identity::api::IdentityId([3; 32]);
    let recipient = crate::modules::identity::api::IdentityId([2; 32]);
    let authority = Arc::new(authority::Authority::new());
    let spend = authority
        .grant(local, authority::Action::Spend, 121)
        .unwrap();
    let context = contract::Context {
        version: 1,
        grant: &spend,
        deadline: Instant::now() + Duration::from_secs(30),
        cancelled: false,
    };
    let request = transfer_crypto::Create {
        issuer,
        new_owner: recipient,
        amount: 9,
        public_key: public.identity.signing_public_key(),
    };
    let signer = transfer_crypto::IdentitySigner {
        identity: &identity,
        handle: &handle,
        capability: &sign,
    };
    let (order, signature) = transfer_crypto::create(
        &context,
        &authority,
        local,
        request,
        &mut SystemEntropy,
        &signer,
    )
    .unwrap();
    transfer_crypto::verify(
        local,
        issuer,
        &order,
        &public.identity.signing_public_key(),
        &signature,
    )
    .unwrap();
    let short_signer = transfer_crypto::IdentitySigner {
        capability: &short,
        ..signer
    };
    assert!(
        transfer_crypto::create(
            &context,
            &authority,
            local,
            request,
            &mut SystemEntropy,
            &short_signer
        )
        .is_err()
    );
    let credential = RecipientCredential {
        identity_id: local.0,
        signing_public_key: public.identity.signing_public_key(),
        encryption_public_key: public.encryption_public_key,
        binding_signature: public.binding_signature,
    };
    let portable = claim::Portable {
        issuer: local,
        id: [4; 32],
        secret: zeroize::Zeroizing::new([5; 32]),
        amount: 7,
    };
    let object = portable
        .encrypt(recipient, &credential, &mut Provider::system(4096).unwrap())
        .unwrap();
    let opener = redemption::IdentityDecryptor {
        identity: &identity,
        handle: &handle,
        capability: &decrypt,
    };
    let control = authority
        .grant(local, authority::Action::Control, 4096)
        .unwrap();
    let context = contract::Context {
        grant: &control,
        ..context
    };
    let parsed =
        redemption::Redemption::decrypt(&context, &authority, local, recipient, &object, &opener)
            .unwrap();
    assert_eq!(parsed.id(), [4; 32]);
    let tiny = identity.grant::<Decrypt>(&admin, &handle, 1).unwrap();
    let opener = redemption::IdentityDecryptor {
        capability: &tiny,
        ..opener
    };
    assert!(
        redemption::Redemption::decrypt(&context, &authority, local, recipient, &object, &opener)
            .is_err()
    );
    drop(identity);
    std::fs::remove_dir_all(path).unwrap();
}
