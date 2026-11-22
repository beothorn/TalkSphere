//! Adapter that lets a protected Identity decrypt Messaging payloads without releasing its private key.
//!
//! Callers: node composition builds one per recipient identity and passes it as the [`super::receive::Recipient`] agreement
//! holder. Dependencies: Identity's scoped `agree` operation (guarded by a `Decrypt` capability) and the payload construction in
//! [`super::encryption`], which keeps the key derivation and authentication. Nothing here can sign or read the key.
use super::{encryption::PersistentAgreement, error::Error};
use crate::modules::{
    cryptography::{
        ports::Entropy,
        secrets::{Secret, Shared},
    },
    identity::{
        authority::{Capability, Decrypt},
        ports::KeyStore,
        provider::{Identity, IdentityHandle},
        record::CredentialRecord,
    },
};

/// A loaded Identity, its handle and a scoped `Decrypt` capability, acting as the recipient's agreement holder.
pub struct IdentityAgreement<'a, S, E> {
    /// The provider holding the protected keys.
    pub identity: &'a Identity<S, E>,
    /// The loaded handle of the recipient identity.
    pub handle: &'a IdentityHandle,
    /// Decrypt authority scoped to that identity; its byte budget must be at least 32.
    pub capability: &'a Capability<Decrypt>,
}

impl<S: KeyStore<Record = CredentialRecord>, E: Entropy> PersistentAgreement
    for IdentityAgreement<'_, S, E>
{
    /// Ask Identity to agree with the sender's ephemeral key; any denial or invalid key is `Invalid`.
    fn agree(&self, remote: &[u8; 32]) -> Result<Secret<Shared>, Error> {
        tracing::trace!("Agreeing through the protected Identity for a message payload");
        self.identity
            .agree(self.handle, self.capability, remote)
            .map_err(|_| Error::Invalid)
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::modules::{
        cryptography::random::SystemEntropy,
        identity::{api::IdentityProvider, config::Limits, storage::FileStore},
        messaging::{
            encryption,
            ids::{MessageId, PayloadHash},
            metadata::Metadata,
            state::Encoding,
            test_support::SequenceEntropy,
        },
    };

    // Scenario: a real protected Identity (owner-only file store) is the recipient. A payload encrypted to its published
    // encryption key decrypts through the adapter; another recipient identity, a lowered budget and a tampered payload fail.
    #[test]
    fn a_protected_identity_decrypts_its_payload_and_nobody_else_does() {
        let dir =
            std::env::temp_dir().join(format!("talksphere-messaging-agree-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let limits = Limits::new(1024, 16, 4096, 4096, 4096).unwrap();
        let (store, admin_a) = FileStore::open(&dir.join("a"), limits).unwrap();
        let mut a = Identity::new(store, SystemEntropy, limits, 1).unwrap();
        let (a_handle, a_public) = a.create(&admin_a).unwrap();
        let (store, admin_b) = FileStore::open(&dir.join("b"), limits).unwrap();
        let mut b = Identity::new(store, SystemEntropy, limits, 1).unwrap();
        let (b_handle, _) = b.create(&admin_b).unwrap();
        let a_cap = a.grant::<Decrypt>(&admin_a, &a_handle, 64).unwrap();
        let _ = &b_handle;
        let metadata = Metadata {
            message_id: MessageId([1; 32]),
            sender_id: a_public.identity.id(),
            recipient_id: a_public.identity.id(),
            sender_created_at: 5,
            encoding: Encoding::IdentityEncryptedV1,
            payload_size: 0,
            payload_hash: PayloadHash([0; 32]),
        };
        let payload = encryption::encrypt(
            b"to the protected identity",
            &metadata,
            &a_public.encryption_public_key,
            &SequenceEntropy::new(1),
        )
        .unwrap();
        let holder = IdentityAgreement {
            identity: &a,
            handle: &a_handle,
            capability: &a_cap,
        };
        let plain = encryption::decrypt(
            &payload,
            &metadata,
            &holder,
            &a_public.encryption_public_key,
        )
        .unwrap();
        assert_eq!(plain.as_slice(), b"to the protected identity");
        let mut tampered = payload.clone();
        *tampered.last_mut().unwrap() ^= 1;
        assert!(
            encryption::decrypt(
                &tampered,
                &metadata,
                &holder,
                &a_public.encryption_public_key
            )
            .is_err()
        );
        // The other identity's key is not the addressed one, so decryption is refused before any agreement.
        let foreign = IdentityAgreement {
            identity: &b,
            handle: &b_handle,
            capability: &a_cap,
        };
        assert!(
            encryption::decrypt(
                &payload,
                &metadata,
                &foreign,
                &a_public.encryption_public_key
            )
            .is_err()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
