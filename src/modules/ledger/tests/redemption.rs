//! Exact owner binding and real generic encryption authentication.
use super::*;
use crate::modules::cryptography::{
    api::{DecryptRequest, KeyAgreement, Signatures},
    credential,
    provider::Provider,
    random::SystemEntropy,
    secrets::{Persistent, Secret},
};
use std::time::{Duration, Instant};
struct Opener {
    crypto: Provider<SystemEntropy>,
    key: Secret<Persistent>,
    id: [u8; 32],
    public: [u8; 32],
}
impl Decryptor for Opener {
    // Exercise the real envelope provider; no stub releases unauthenticated plaintext.
    fn decrypt(&self, object: &[u8]) -> Result<Zeroizing<Vec<u8>>, Error> {
        self.crypto
            .decrypt(DecryptRequest {
                object,
                identity_id: self.id,
                encryption_public_key: self.public,
                private_key: &self.key,
                limits: ResourceLimits::new(4096, 4096).unwrap(),
            })
            .map(Zeroizing::new)
            .map_err(|_| Error::Denied)
    }
}
// Tampering, wrong recipient/caller and malformed lengths never return a usable redemption.
#[test]
fn encrypted_owner_binding() {
    let mut crypto = Provider::system(4096).unwrap();
    let signing = crypto.generate_signing_key().unwrap();
    let encryption = crypto.generate_persistent_key().unwrap();
    let id = credential::identity_id(&signing.public_key).unwrap();
    let credential = RecipientCredential {
        identity_id: id,
        signing_public_key: signing.public_key,
        encryption_public_key: encryption.public_key,
        binding_signature: credential::bind(&signing.private_key, &encryption.public_key).unwrap(),
    };
    let owner = IdentityId([2; 32]);
    let claim = Portable {
        issuer: IdentityId(id),
        id: [4; 32],
        secret: Zeroizing::new([5; 32]),
        amount: 7,
    };
    let object = claim.encrypt(owner, &credential, &mut crypto).unwrap();
    assert_eq!(object.len(), ENVELOPE_BYTES);
    assert!(!object.windows(32).any(|w| w == [5; 32]));
    let opener = Opener {
        crypto,
        key: encryption.private_key,
        id,
        public: encryption.public_key,
    };
    let auth = Authority::new();
    let control = auth.grant(IdentityId(id), Action::Control, 4096).unwrap();
    let context = Context {
        version: 1,
        grant: &control,
        deadline: Instant::now() + Duration::from_secs(30),
        cancelled: false,
    };
    let result =
        Redemption::decrypt(&context, &auth, IdentityId(id), owner, &object, &opener).unwrap();
    assert_eq!(result.id(), [4; 32]);
    assert_eq!(result.owner(), owner);
    assert_eq!(result.secret.as_ref(), &[5; 32]);
    for at in [0, 4, 36, 68, 100, 132, object.len() - 1] {
        let mut bad = object.clone();
        bad[at] ^= 1;
        assert!(
            Redemption::decrypt(&context, &auth, IdentityId(id), owner, &bad, &opener).is_err()
        );
    }
    assert!(
        Redemption::decrypt(
            &context,
            &auth,
            IdentityId(id),
            IdentityId([3; 32]),
            &object,
            &opener
        )
        .is_err()
    );
    assert!(
        Redemption::decrypt(
            &context,
            &auth,
            IdentityId(id),
            owner,
            &object[..object.len() - 1],
            &opener
        )
        .is_err()
    );
}
