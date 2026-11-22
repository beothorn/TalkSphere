//! The Messaging participant: durable preparation, idempotent commit, abort and status of accepted effects.
//!
//! Callers: the Services coordinator (through the provider adapter). The coordinator prepares an effect, records a
//! durable decision, then commits or aborts it; after any crash it asks for status instead of repeating work. This
//! participant keeps one journal entry per operation key. Commit applies the effect at most once: applying is
//! idempotent (an already applied effect is recognized from the placement itself), so a crash between applying and
//! journaling is repaired by committing again. A placement touched by a prepared effect is locked until that effect
//! ends, so two accepted operations on one placement can never both prepare. Timestamps (`acceptedAt`, `recoveredAt`) are
//! taken at the durable commit, never at preparation.
use super::{
    effect::{Effect, Outcome},
    error::Error,
    ids::{PlacementId, RemovalId},
    journal::{Entry, Journal},
    lifecycle,
    placement::{Placement, Record, Removal},
    ports::{Clock, Entropy, Signer},
    receipt::{self, Binding},
    state::State,
    store::Store,
};
use crate::modules::{identity::api::IdentityId, services::contract::OperationKey};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

/// Durable participant state, as the coordinator observes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// No staged operation is known.
    Absent,
    /// The effect is prepared; this token commits it.
    Prepared([u8; 32]),
    /// The effect was applied once; the outcome is retained.
    Committed(Box<Outcome>),
    /// The coordinator aborted it; nothing was applied.
    Aborted,
}

/// The participant.
pub struct Participant {
    store: Arc<Store>,
    journal: Journal,
    entropy: Arc<dyn Entropy>,
    clock: Arc<dyn Clock>,
    signer: Arc<dyn Signer>,
    /// Placements locked by a prepared effect, and which operation holds each.
    locks: Mutex<BTreeMap<PlacementId, OperationKey>>,
    ready: AtomicBool,
}

impl Participant {
    /// A participant over a store and journal. It admits no new work until recovery has completed.
    pub fn new(
        store: Arc<Store>,
        journal: Journal,
        entropy: Arc<dyn Entropy>,
        clock: Arc<dyn Clock>,
        signer: Arc<dyn Signer>,
    ) -> Self {
        tracing::trace!("Creating the Messaging participant");
        let participant = Self {
            store,
            journal,
            entropy,
            clock,
            signer,
            locks: Mutex::new(BTreeMap::new()),
            ready: AtomicBool::new(false),
        };
        // Prepared effects found in the journal keep their placement locked until the coordinator decides.
        let mut locks = participant.locks();
        for (key, effect, _) in participant.journal.prepared() {
            if let Some(id) = effect.placement() {
                locks.insert(id, key);
            }
        }
        drop(locks);
        participant
    }

    /// Whether recovery has completed and new work is admitted.
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }

    /// Admit new work. Called only by recovery once every prepared effect is resolved.
    pub(super) fn set_ready(&self) {
        tracing::trace!("Admitting new Messaging work after recovery");
        self.ready.store(true, Ordering::SeqCst);
    }

    /// The identity this provider signs receipts as.
    pub fn provider_id(&self) -> IdentityId {
        self.signer.identity()
    }

    /// The store, for read-only queries by the provider engine.
    pub fn store(&self) -> &Arc<Store> {
        &self.store
    }

    /// A fresh provider-chosen placement identifier from secure entropy.
    /// # Errors
    /// `Unavailable` when randomness is missing.
    pub fn new_placement_id(&self) -> Result<PlacementId, Error> {
        tracing::trace!("Choosing a placement identifier");
        let mut bytes = [0u8; 32];
        self.entropy.fill(&mut bytes)?;
        Ok(PlacementId(bytes))
    }

    /// Durably prepare an effect and return the token that authorizes its commit. Preparing the same effect again returns the same token.
    /// # Errors
    /// `Unavailable` before recovery completes, `Conflict` for a different effect under the same key or a placement already locked by
    /// another operation, `NotAvailable`/`Conflict`/`Limit` when the effect's preconditions do not hold now, `Storage` on a failed write.
    pub fn prepare(&self, key: OperationKey, effect: &Effect) -> Result<[u8; 32], Error> {
        tracing::trace!("Preparing a Messaging effect");
        if let Some(entry) = self.journal.get(key) {
            return match entry {
                Entry::Prepared {
                    effect: existing,
                    token,
                } if existing == *effect => Ok(token),
                _ => Err(Error::Conflict),
            };
        }
        if !self.is_ready() {
            return Err(Error::Unavailable);
        }
        let now = self.clock.now()?;
        self.check_preconditions(effect, now)?;
        let mut token = [0u8; 32];
        self.entropy.fill(&mut token)?;
        let mut locks = self.locks();
        if let Some(id) = effect.placement() {
            if locks.get(&id).is_some_and(|holder| *holder != key) {
                return Err(Error::Conflict);
            }
            locks.insert(id, key);
        }
        if let Err(error) = self.journal.put(
            key,
            Entry::Prepared {
                effect: effect.clone(),
                token,
            },
        ) {
            if let Some(id) = effect.placement() {
                locks.remove(&id);
            }
            return Err(error);
        }
        Ok(token)
    }

    /// Apply a prepared effect at most once and retain its outcome. Committing again returns the retained outcome.
    /// # Errors
    /// `NotAvailable` for an unknown key or a token that is not the prepared one, `Conflict` for an aborted operation, otherwise the
    /// failure of applying (the effect stays prepared and committing again is safe), or `Storage` when the journal write fails after the
    /// effect was applied (committing again completes it).
    pub fn commit(&self, key: OperationKey, token: &[u8; 32]) -> Result<Outcome, Error> {
        tracing::trace!("Committing a Messaging effect");
        match self.journal.get(key) {
            None => Err(Error::NotAvailable),
            Some(Entry::Aborted) => Err(Error::Conflict),
            Some(Entry::Committed(outcome)) => Ok(outcome),
            Some(Entry::Prepared {
                token: expected, ..
            }) if expected != *token => Err(Error::NotAvailable),
            Some(Entry::Prepared { effect, .. }) => {
                let outcome = self.apply(&effect)?;
                self.journal.put(key, Entry::Committed(outcome.clone()))?;
                if let Some(id) = effect.placement() {
                    self.locks().remove(&id);
                }
                Ok(outcome)
            }
        }
    }

    /// Release a prepared effect that the coordinator aborted. Aborting an absent or already aborted operation is a no-op.
    /// # Errors
    /// `Conflict` for a committed operation, `NotAvailable` for a token that is not the prepared one, `Storage` on a failed write.
    pub fn abort(&self, key: OperationKey, token: Option<&[u8; 32]>) -> Result<(), Error> {
        tracing::trace!("Aborting a Messaging effect");
        match self.journal.get(key) {
            None | Some(Entry::Aborted) => Ok(()),
            Some(Entry::Committed(_)) => Err(Error::Conflict),
            Some(Entry::Prepared {
                token: expected,
                effect,
            }) => {
                if token.is_some_and(|t| *t != expected) {
                    return Err(Error::NotAvailable);
                }
                self.journal.put(key, Entry::Aborted)?;
                if let Some(id) = effect.placement() {
                    self.locks().remove(&id);
                }
                Ok(())
            }
        }
    }

    /// The durable state of an operation.
    pub fn status(&self, key: OperationKey) -> Status {
        tracing::trace!("Reading a Messaging participant status");
        match self.journal.get(key) {
            None => Status::Absent,
            Some(Entry::Prepared { token, .. }) => Status::Prepared(token),
            Some(Entry::Committed(outcome)) => Status::Committed(Box::new(outcome)),
            Some(Entry::Aborted) => Status::Aborted,
        }
    }

    /// Every prepared operation, for recovery.
    pub fn prepared(&self) -> Vec<(OperationKey, [u8; 32])> {
        tracing::trace!("Listing prepared Messaging operations");
        self.journal
            .prepared()
            .into_iter()
            .map(|(key, _, token)| (key, token))
            .collect()
    }

    // Check, without changing anything, that the effect could be applied now.
    fn check_preconditions(&self, effect: &Effect, now: u64) -> Result<(), Error> {
        match effect {
            Effect::Query => Ok(()),
            Effect::Store {
                placement_id,
                message,
                ..
            } => {
                if self.store.record(*placement_id).is_some() {
                    return Err(Error::Conflict);
                }
                if self.store.has_room(message.payload.len() as u64) {
                    Ok(())
                } else {
                    Err(Error::Limit)
                }
            }
            Effect::Recover { input, .. } => {
                let record = lifecycle::touch(&self.store, input.placement_id, now)?;
                if record.state == State::Pending {
                    Ok(())
                } else {
                    Err(Error::NotAvailable)
                }
            }
            Effect::Renew {
                placement_id,
                expected_expires_at,
                ..
            } => {
                let record = lifecycle::touch(&self.store, *placement_id, now)?;
                if !record.active_at(now) {
                    return Err(Error::NotAvailable);
                }
                if record.expires_at != *expected_expires_at {
                    Err(Error::Conflict)
                } else {
                    Ok(())
                }
            }
            Effect::Remove { input, .. } => {
                let record = lifecycle::touch(&self.store, input.placement_id, now)?;
                if record.state != State::Pending {
                    return Err(Error::NotAvailable);
                }
                if record.expires_at != input.expected_expires_at {
                    Err(Error::Conflict)
                } else {
                    Ok(())
                }
            }
        }
    }

    // Apply an effect idempotently: an effect already visible in the store is recognized and its outcome rebuilt.
    fn apply(&self, effect: &Effect) -> Result<Outcome, Error> {
        let now = self.clock.now()?;
        match effect {
            Effect::Query => Ok(Outcome::Query),
            Effect::Store {
                placement_id,
                message,
                retention_seconds,
                price,
            } => {
                if let Some(existing) = self.store.record(*placement_id) {
                    // Recognize our own earlier application; anything else is a collision.
                    if existing.metadata != message.metadata {
                        return Err(Error::Conflict);
                    }
                    let expires = existing
                        .accepted_at
                        .checked_add(*retention_seconds)
                        .ok_or(Error::Limit)?;
                    return Ok(Outcome::Stored {
                        placement_id: *placement_id,
                        message_id: message.metadata.message_id,
                        accepted_at: existing.accepted_at,
                        expires_at: expires,
                        price: *price,
                    });
                }
                let expires = now.checked_add(*retention_seconds).ok_or(Error::Limit)?;
                self.store.insert(&Placement {
                    record: Record {
                        id: *placement_id,
                        metadata: message.metadata.clone(),
                        sender_public_key: message.sender_public_key,
                        sender_signature: message.sender_signature,
                        accepted_at: now,
                        expires_at: expires,
                        state: State::Pending,
                        payments: *price,
                        recovery_price: 0,
                        receipt: None,
                        removal: None,
                    },
                    payload: message.payload.clone(),
                })?;
                Ok(Outcome::Stored {
                    placement_id: *placement_id,
                    message_id: message.metadata.message_id,
                    accepted_at: now,
                    expires_at: expires,
                    price: *price,
                })
            }
            Effect::Recover { input, price } => {
                let record = self
                    .store
                    .record(input.placement_id)
                    .ok_or(Error::NotAvailable)?;
                if let (State::Recovered, Some(done)) = (record.state, record.receipt)
                    && done.recovery_id == input.recovery_id
                {
                    return Ok(Outcome::Recovered {
                        placement_id: input.placement_id,
                        receipt: done,
                        price: *price,
                    });
                }
                let binding = Binding {
                    provider_id: self.signer.identity(),
                    placement_id: input.placement_id,
                    message_id: record.metadata.message_id,
                    sender_id: record.metadata.sender_id,
                    recipient_id: record.metadata.recipient_id,
                    payload_hash: record.metadata.payload_hash,
                };
                let provider_signature =
                    receipt::sign_receipt(self.signer.as_ref(), &binding, input.recovery_id, now)?;
                let receipt = super::operation::Receipt {
                    recovery_id: input.recovery_id,
                    recovered_at: now,
                    recipient_public_key: input.recipient_public_key,
                    recipient_signature: input.recipient_signature,
                    provider_signature,
                };
                lifecycle::mark_recovered(&self.store, input.placement_id, now, receipt, *price)?;
                Ok(Outcome::Recovered {
                    placement_id: input.placement_id,
                    receipt,
                    price: *price,
                })
            }
            Effect::Renew {
                placement_id,
                expected_expires_at,
                additional_seconds,
                ceiling_expires_at,
                price,
            } => {
                let record = self
                    .store
                    .record(*placement_id)
                    .ok_or(Error::NotAvailable)?;
                let target = expected_expires_at
                    .checked_add(*additional_seconds)
                    .ok_or(Error::Limit)?;
                // The expiry only ever grows, so a later expiry means this renewal (or a later one) was already applied.
                if record.expires_at < target {
                    lifecycle::extend(
                        &self.store,
                        *placement_id,
                        now,
                        *expected_expires_at,
                        *additional_seconds,
                        *ceiling_expires_at,
                        *price,
                    )?;
                }
                Ok(Outcome::Renewed {
                    placement_id: *placement_id,
                    previous_expires_at: *expected_expires_at,
                    expires_at: target,
                    price: *price,
                })
            }
            Effect::Remove {
                input,
                release_credits,
                price,
            } => {
                let record = self
                    .store
                    .record(input.placement_id)
                    .ok_or(Error::NotAvailable)?;
                if let (State::Removed, Some(done)) = (record.state, record.removal)
                    && done.removal_id == input.removal_id
                {
                    return Ok(Self::removed(input.placement_id, done));
                }
                let removal = Removal {
                    removal_id: input.removal_id,
                    release_credits: *release_credits,
                    price: *price,
                };
                lifecycle::mark_removed(
                    &self.store,
                    input.placement_id,
                    now,
                    input.expected_expires_at,
                    removal,
                )?;
                // The payload is no longer owed once the placement is removed.
                self.store.release_payload(input.placement_id)?;
                Ok(Self::removed(input.placement_id, removal))
            }
        }
    }

    fn removed(placement_id: PlacementId, removal: Removal) -> Outcome {
        Outcome::Removed {
            placement_id,
            removal_id: RemovalId(removal.removal_id.0),
            release_credits: removal.release_credits,
            price: removal.price,
        }
    }

    fn locks(&self) -> std::sync::MutexGuard<'_, BTreeMap<PlacementId, OperationKey>> {
        self.locks.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
#[path = "participant_tests.rs"]
mod tests;
