//! Requester-side acceptance of a recovered message: authenticate first, deduplicate, decrypt last.
//!
//! Callers: the recipient's Messaging client after a recovery. The recipient never trusts what a provider returns: the sender's key
//! must derive to the sender ID, the payload size and hash must match and the sender's signature must verify; the recovery receipt
//! must carry the recipient's own authorization and the provider's signature for exactly this placement. Only then is the message
//! compared with what was already received, keyed by `(senderId, messageId)`, so the same message recovered from several providers
//! (replicas) is delivered once and a sender that signs two different payloads under one ID is reported rather than shown. Decryption
//! of `cr2se.identity-encrypted.v1` payloads happens last, so unauthenticated bytes never reach the cipher.
//! Dependencies: signing, receipt and encryption; the dedupe table here is in memory, and the client catalog that persists it belongs
//! to the client journey.
use super::{
    encryption,
    error::Error,
    ids::{MessageId, PayloadHash},
    operation::RecoverOutput,
    receipt::{self, Binding},
    signing,
    state::Encoding,
};
use crate::modules::{
    cryptography::secrets::{Persistent, Secret},
    identity::api::IdentityId,
};
use std::{collections::BTreeMap, sync::Mutex};
use zeroize::Zeroizing;

/// The recipient's own identity and X25519 decryption key.
pub struct Recipient<'a> {
    /// The recipient identity the message must be addressed to.
    pub id: IdentityId,
    /// The persistent private key matching the published `identityEncryption` credential.
    pub encryption_key: &'a Secret<Persistent>,
    /// The matching X25519 public key.
    pub encryption_public_key: [u8; 32],
}

/// The provider that returned a message, as known from its Board and authenticated connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderKey {
    /// The provider's identity.
    pub id: IdentityId,
    /// The provider's Ed25519 public key, which must derive to `id`.
    pub public_key: [u8; 32],
}

/// A message that passed every check.
pub struct Received {
    /// The verified sender.
    pub sender_id: IdentityId,
    /// The message identifier shared by replicas.
    pub message_id: MessageId,
    /// The sender-claimed creation time.
    pub sender_created_at: u64,
    /// The payload encoding as stored.
    pub encoding: Encoding,
    /// The decrypted content for encrypted messages, or the opaque payload bytes.
    pub content: Zeroizing<Vec<u8>>,
}

impl std::fmt::Debug for Received {
    // Message content is private and never printed.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Received")
            .field("message_id", &self.message_id)
            .field("content", &format_args!("{} bytes", self.content.len()))
            .finish_non_exhaustive()
    }
}

/// What accepting a recovered message produced.
#[derive(Debug)]
pub enum Arrival {
    /// A message not seen before, authenticated and decrypted.
    New(Received),
    /// The same message (same sender, ID and payload hash) was already accepted, for example from another provider's replica.
    Duplicate,
}

/// The recipient's record of accepted messages, bounded in size.
pub struct Inbox {
    maximum: usize,
    seen: Mutex<BTreeMap<(IdentityId, MessageId), PayloadHash>>,
}

impl Inbox {
    /// An empty inbox that remembers at most `maximum` messages.
    pub fn new(maximum: usize) -> Self {
        tracing::trace!("Creating a recovered message inbox");
        Self {
            maximum,
            seen: Mutex::new(BTreeMap::new()),
        }
    }

    /// How many distinct messages were accepted.
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// Whether nothing was accepted yet.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Authenticate a recovery result, deduplicate it and decrypt it.
    /// # Errors
    /// `Invalid` for any failed check (sender key, size, hash, signature, recipient, receipt, provider key or decryption) with nothing
    /// remembered; `Conflict` when the same `(senderId, messageId)` arrives with a different payload hash (the sender signed two
    /// messages under one ID; nothing is delivered); `Limit` when the inbox is full.
    pub fn accept(
        &self,
        recipient: &Recipient<'_>,
        provider: &ProviderKey,
        output: &RecoverOutput,
    ) -> Result<Arrival, Error> {
        tracing::trace!("Accepting a recovered message");
        let message = &output.message;
        let metadata = &message.metadata;
        signing::verify_message(message, None)?;
        if metadata.recipient_id != recipient.id {
            return Err(Error::Invalid);
        }
        let binding = Binding {
            provider_id: provider.id,
            placement_id: output.placement_id,
            message_id: metadata.message_id,
            sender_id: metadata.sender_id,
            recipient_id: metadata.recipient_id,
            payload_hash: metadata.payload_hash,
        };
        receipt::verify_receipt(&output.receipt, &binding, &provider.public_key)?;
        let key = (metadata.sender_id, metadata.message_id);
        {
            let seen = self.lock();
            match seen.get(&key) {
                Some(hash) if *hash == metadata.payload_hash => return Ok(Arrival::Duplicate),
                Some(_) => return Err(Error::Conflict),
                None if seen.len() >= self.maximum => return Err(Error::Limit),
                None => {}
            }
        }
        let content = match metadata.encoding {
            Encoding::Opaque => Zeroizing::new(message.payload.clone()),
            Encoding::IdentityEncryptedV1 => encryption::decrypt(
                &message.payload,
                metadata,
                recipient.encryption_key,
                &recipient.encryption_public_key,
            )?,
        };
        // Remembered only after decryption succeeded, so a message that cannot be read yet can be accepted later.
        if self
            .lock()
            .insert(key, metadata.payload_hash)
            .is_some_and(|hash| hash != metadata.payload_hash)
        {
            return Err(Error::Conflict);
        }
        Ok(Arrival::New(Received {
            sender_id: metadata.sender_id,
            message_id: metadata.message_id,
            sender_created_at: metadata.sender_created_at,
            encoding: metadata.encoding,
            content,
        }))
    }

    // A poisoned lock still holds a consistent map because every change is one insert.
    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<(IdentityId, MessageId), PayloadHash>> {
        self.seen.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
#[path = "receive_tests.rs"]
mod tests;
