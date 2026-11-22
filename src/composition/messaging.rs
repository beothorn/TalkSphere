//! Production ports for the Messaging module over a node's protected identity, the system clock and system entropy.
//!
//! Callers: node composition and the Messaging exercise, which build the participant, the recipient's payload decryption and
//! message/recovery signing from one [`ProtectedIdentity`]. Dependencies: the shared identity (which keeps private keys inside
//! Identity), `std::time` and the Cryptography system entropy. Nothing here persists state or reaches the network.
use super::identity::ProtectedIdentity;
use crate::modules::{
    cryptography::{
        ports::Entropy as CryptoEntropy,
        random::SystemEntropy,
        secrets::{Secret, Shared},
    },
    identity::{api::IdentityId, error::IdentityError, ports::KeyStore, record::CredentialRecord},
    messaging::{
        encryption::PersistentAgreement,
        error::Error,
        ids::Signature,
        ports::{Clock, Entropy, Signer},
    },
};
use std::time::{SystemTime, UNIX_EPOCH};

/// The node identity as a Messaging signer (receipts, messages and recovery requests) and payload agreement holder.
pub struct NodeIdentity<S, E>(pub ProtectedIdentity<S, E>);

impl<S: KeyStore<Record = CredentialRecord> + Send + Sync, E: CryptoEntropy + Send + Sync> Signer
    for NodeIdentity<S, E>
{
    /// The local identity identifier; unavailable only if the credential cannot be read.
    fn identity(&self) -> IdentityId {
        tracing::trace!("Reading the node identity for Messaging");
        self.0
            .public()
            .map(|public| public.identity.id())
            .unwrap_or(IdentityId([0; 32]))
    }
    /// The Ed25519 public key of the node identity (all zero if unreadable, which no signature verifies against).
    fn public_key(&self) -> [u8; 32] {
        tracing::trace!("Reading the node signing key for Messaging");
        self.0
            .public()
            .map(|public| public.identity.signing_public_key())
            .unwrap_or([0; 32])
    }
    /// Sign exactly `message` through the protected identity.
    /// # Errors
    /// `Unavailable` when Identity refuses or cannot reach the key.
    fn sign(&self, message: &[u8]) -> Result<Signature, Error> {
        tracing::trace!("Signing Messaging bytes with the node identity");
        self.0.sign(message).map(Signature).map_err(unavailable)
    }
}

impl<S: KeyStore<Record = CredentialRecord> + Send + Sync, E: CryptoEntropy + Send + Sync>
    PersistentAgreement for NodeIdentity<S, E>
{
    /// Agree through the protected identity; every refusal is `Invalid` so a payload is never partially read.
    fn agree(&self, remote: &[u8; 32]) -> Result<Secret<Shared>, Error> {
        tracing::trace!("Agreeing for a Messaging payload through the node identity");
        self.0.agree(remote).map_err(|_| Error::Invalid)
    }
}

// Identity failures never reveal which condition denied the key.
fn unavailable(_: IdentityError) -> Error {
    Error::Unavailable
}

/// Unix seconds from the operating system clock.
pub struct SystemClock;
impl Clock for SystemClock {
    /// # Errors
    /// `Unavailable` when the system clock is before the Unix epoch.
    fn now(&self) -> Result<u64, Error> {
        tracing::trace!("Reading the system clock for Messaging");
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .map_err(|_| Error::Unavailable)
    }
}

/// Operating-system randomness for Messaging identifiers and payload encryption.
pub struct SystemRandom;
impl Entropy for SystemRandom {
    /// # Errors
    /// `Unavailable` when the system cannot provide randomness; no fallback is used.
    fn fill(&self, bytes: &mut [u8]) -> Result<(), Error> {
        tracing::trace!("Filling Messaging randomness from the system");
        SystemEntropy.fill(bytes).map_err(|_| Error::Unavailable)
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::{
        composition::server::{
            config::{Config, Options},
            credentials,
        },
        modules::{
            identity::{
                api::IdentityProvider, config::Limits, provider::Identity, storage::FileStore,
            },
            messaging::{
                encryption,
                ids::{MessageId, PayloadHash},
                metadata::Metadata,
                signing,
                state::Encoding,
            },
        },
    };

    // Scenario: a node identity created in an owner-only store and loaded as a server credential signs messages that verify, and
    // decrypts a payload encrypted to its published key; the clock and system randomness are usable.
    #[test]
    fn a_loaded_node_identity_signs_and_decrypts_for_messaging() {
        let path =
            std::env::temp_dir().join(format!("talksphere-node-messaging-{}", std::process::id()));
        // The protected store insists on an owner-only directory.
        std::os::unix::fs::DirBuilderExt::mode(&mut std::fs::DirBuilder::new(), 0o700)
            .create(&path)
            .unwrap();
        let limits = Limits::new(1024, 64, 65548, 65536, 65536).unwrap();
        {
            let (store, admin) = FileStore::open(&path, limits).unwrap();
            let mut provider = Identity::new(store, SystemEntropy, limits, 1).unwrap();
            provider.create(&admin).unwrap();
        }
        let config = Config::new(Options::local(path.clone()).unwrap()).unwrap();
        let loaded = credentials::load(&config, false).unwrap();
        let node = NodeIdentity(loaded.identity.protected());
        let public = node.0.public().unwrap();
        assert_eq!(node.identity(), public.identity.id());
        assert_eq!(node.public_key(), public.identity.signing_public_key());

        let message = signing::sign_message(
            &node,
            node.identity(),
            MessageId([1; 32]),
            SystemClock.now().unwrap(),
            Encoding::Opaque,
            b"signed by the node".to_vec(),
        )
        .unwrap();
        signing::verify_message(&message, Some(node.identity())).unwrap();

        let metadata = Metadata {
            message_id: MessageId([2; 32]),
            sender_id: node.identity(),
            recipient_id: node.identity(),
            sender_created_at: 5,
            encoding: Encoding::IdentityEncryptedV1,
            payload_size: 0,
            payload_hash: PayloadHash([0; 32]),
        };
        let payload = encryption::encrypt(
            b"to myself",
            &metadata,
            &public.encryption_public_key,
            &SystemRandom,
        )
        .unwrap();
        let plain =
            encryption::decrypt(&payload, &metadata, &node, &public.encryption_public_key).unwrap();
        assert_eq!(plain.as_slice(), b"to myself");
        let mut damaged = payload;
        *damaged.last_mut().unwrap() ^= 1;
        assert!(
            encryption::decrypt(&damaged, &metadata, &node, &public.encryption_public_key).is_err()
        );
        let _ = std::fs::remove_dir_all(&path);
    }

    // Scenario: the clock reports a plausible current time and system randomness differs between draws.
    #[test]
    fn the_system_clock_and_randomness_are_usable() {
        assert!(SystemClock.now().unwrap() > 1_700_000_000);
        let (mut a, mut b) = ([0u8; 32], [0u8; 32]);
        SystemRandom.fill(&mut a).unwrap();
        SystemRandom.fill(&mut b).unwrap();
        assert_ne!(a, b);
    }
}
