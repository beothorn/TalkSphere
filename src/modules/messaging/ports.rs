//! Dependencies Messaging receives from composition.
//!
//! Callers: the provider and requester engines. Production composition supplies platform
//! implementations; tests supply deterministic ones. Nothing here may block indefinitely or log secrets.
use super::{error::Error, ids::Signature};
use crate::modules::{identity::api::IdentityId, services::contract::OperationKey};

/// Provider wall-clock time in whole Unix seconds.
pub trait Clock: Send + Sync {
    /// The current Unix time.
    /// # Errors
    /// `Unavailable` when no trustworthy time exists; no operation proceeds without one.
    fn now(&self) -> Result<u64, Error>;
}

/// Cryptographically secure randomness for identifiers.
pub trait Entropy: Send + Sync {
    /// Fill `bytes` with unpredictable bytes.
    /// # Errors
    /// `Unavailable` when randomness cannot be obtained; no identifier is ever derived from a fallback.
    fn fill(&self, bytes: &mut [u8]) -> Result<(), Error>;
}

/// Signing with the provider's identity key, used only for recovery receipts.
pub trait Signer: Send + Sync {
    /// The identity that owns the signing key.
    fn identity(&self) -> IdentityId;
    /// The identity's 32-byte Ed25519 public key.
    fn public_key(&self) -> [u8; 32];
    /// Sign exactly the given bytes.
    /// # Errors
    /// `Unavailable` when the key is not accessible.
    fn sign(&self, message: &[u8]) -> Result<Signature, Error>;
}

/// Chooses the early-release credits offered for removing a placement; the node fills this from its peer policy.
pub trait ReleasePolicy: Send + Sync {
    /// Credits to offer given the placement's cumulative store and renewal payments and the smallest amount the sender
    /// accepts. The module bounds the answer by the payments itself, so a policy cannot enlarge the ceiling.
    fn release(&self, cumulative_payments: u64, minimum: u64) -> u64;
}

impl<F: Fn(u64, u64) -> u64 + Send + Sync> ReleasePolicy for F {
    /// A closure over `(cumulative_payments, minimum)` is a policy, so composition can adapt the node's credit policy directly.
    fn release(&self, cumulative_payments: u64, minimum: u64) -> u64 {
        self(cumulative_payments, minimum)
    }
}

/// A policy that never offers release credits.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoRelease;

impl ReleasePolicy for NoRelease {
    /// Always zero.
    fn release(&self, _cumulative_payments: u64, _minimum: u64) -> u64 {
        0
    }
}

/// Issues early-release credits to a sender, once per Services operation.
pub trait ReleaseIssuer: Send + Sync {
    /// Issue `credits` to `recipient` for the operation `key`. Repeating the exact call must succeed without issuing again, so a
    /// recovered removal can finish a settlement that crashed between its records.
    /// # Errors
    /// `Insufficient` or `Limit` when the issuer cannot issue them, `Conflict` when `key` issued something different, `Storage` or
    /// `Unknown` when the record may or may not be durable.
    fn issue(&self, key: OperationKey, recipient: IdentityId, credits: u64) -> Result<(), Error>;
}

/// Bounds on pending placements so unsolicited messaging stays finite at each provider.
pub trait PendingLimits: Send + Sync {
    /// Most pending placements one sender may hold at this provider.
    fn per_sender(&self, sender: IdentityId) -> usize;
    /// Most pending placements one recipient may have waiting at this provider.
    fn per_recipient(&self, recipient: IdentityId) -> usize;
}

/// The same finite bounds for every identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedLimits {
    /// Most pending placements per sender.
    pub per_sender: usize,
    /// Most pending placements per recipient.
    pub per_recipient: usize,
}

impl Default for FixedLimits {
    /// 1024 per sender and per recipient.
    fn default() -> Self {
        Self {
            per_sender: 1024,
            per_recipient: 1024,
        }
    }
}

impl PendingLimits for FixedLimits {
    /// The configured per-sender bound.
    fn per_sender(&self, _sender: IdentityId) -> usize {
        self.per_sender
    }
    /// The configured per-recipient bound.
    fn per_recipient(&self, _recipient: IdentityId) -> usize {
        self.per_recipient
    }
}
