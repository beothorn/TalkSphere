//! The hosted publication participant: durable preparation, idempotent commit, abort and status of accepted effects.
//!
//! Callers: the Services coordinator (through the provider adapter). The coordinator prepares an effect, records a durable decision, then
//! commits or aborts it; after any crash it asks for status instead of repeating work. This participant keeps one journal entry per
//! operation key. Commit applies the effect at most once through the provider engine, whose commits are idempotent (an already applied
//! effect is recognized from the placement itself), so a crash between applying and journaling is repaired by committing again. The
//! things an effect touches (its placement, and for a store also its publication ID and its page or feed sequence slot) are locked until
//! the effect ends, so two accepted operations that would conflict can never both prepare. Timestamps are taken at the durable commit,
//! never at preparation.
use super::{
    effect::{Effect, Outcome},
    error::Error,
    ids::{PlacementId, PublicationId},
    journal::{Entry, Journal},
    lifecycle,
    provider::{Provider, RemovalQuote, RenewPlan, StorePlan},
};
use crate::modules::{
    identity::api::IdentityId, messaging::ports::Entropy, services::contract::OperationKey,
};
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

// Something an effect holds exclusively until it ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Lock {
    Placement(PlacementId),
    Publication(IdentityId, PublicationId),
    // Publisher, kind code and sequence of a page or feed post.
    Sequence(IdentityId, u8, u64),
}

/// The participant.
pub struct Participant {
    provider: Arc<Provider>,
    journal: Journal,
    entropy: Arc<dyn Entropy>,
    /// Things locked by a prepared effect, and which operation holds each.
    locks: Mutex<BTreeMap<Lock, OperationKey>>,
    ready: AtomicBool,
}

impl Participant {
    /// A participant over a provider engine and a journal. It admits no new work until recovery has completed.
    pub fn new(provider: Arc<Provider>, journal: Journal, entropy: Arc<dyn Entropy>) -> Self {
        tracing::trace!("Creating the hosted publication participant");
        let participant = Self {
            provider,
            journal,
            entropy,
            locks: Mutex::new(BTreeMap::new()),
            ready: AtomicBool::new(false),
        };
        // Prepared effects found in the journal keep their locks until the coordinator decides.
        let mut locks = participant.locks();
        for (key, effect, _) in participant.journal.prepared() {
            for lock in lock_set(&effect) {
                locks.insert(lock, key);
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
        tracing::trace!("Admitting new hosted publication work after recovery");
        self.ready.store(true, Ordering::SeqCst);
    }

    /// The provider engine this participant applies effects through.
    pub fn provider(&self) -> &Arc<Provider> {
        &self.provider
    }

    /// The effect journaled as prepared under `key`, if any.
    pub fn prepared_effect(&self, key: OperationKey) -> Option<Effect> {
        tracing::trace!("Reading a prepared effect");
        match self.journal.get(key) {
            Some(Entry::Prepared { effect, .. }) => Some(effect),
            _ => None,
        }
    }

    /// Durably prepare an effect and return the token that authorizes its commit. Preparing the same effect again returns the same token.
    /// # Errors
    /// `Unavailable` before recovery completes, `Conflict` for a different effect under the same key or something already locked by
    /// another operation, `NotAvailable`/`Conflict`/`Limit` when the effect's preconditions do not hold now, `Storage` on a failed write.
    pub fn prepare(&self, key: OperationKey, effect: &Effect) -> Result<[u8; 32], Error> {
        tracing::trace!("Preparing a hosted publication effect");
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
        let now = self.provider.now()?;
        let mut token = [0u8; 32];
        self.entropy.fill(&mut token)?;
        let wanted = lock_set(effect);
        let mut locks = self.locks();
        if wanted
            .iter()
            .any(|lock| locks.get(lock).is_some_and(|holder| *holder != key))
        {
            return Err(Error::Conflict);
        }
        // Preconditions are judged while holding the lock table, after any earlier operation on the same things has finished: otherwise
        // a second operation could pass them just before the first applied and then be mistaken for a replay of it.
        self.check_preconditions(effect, now)?;
        for lock in &wanted {
            locks.insert(*lock, key);
        }
        if let Err(error) = self.journal.put(
            key,
            Entry::Prepared {
                effect: effect.clone(),
                token,
            },
        ) {
            for lock in &wanted {
                locks.remove(lock);
            }
            return Err(error);
        }
        Ok(token)
    }

    /// Apply a prepared effect at most once at the current time and retain its outcome. Committing again returns the retained outcome.
    /// # Errors
    /// As for [`Participant::commit_at`], plus `Unavailable` without a clock.
    pub fn commit(&self, key: OperationKey, token: &[u8; 32]) -> Result<Outcome, Error> {
        tracing::trace!("Committing a hosted publication effect");
        let now = self.provider.now()?;
        self.commit_at(key, token, now)
    }

    /// Like [`Participant::commit`] but applying the effect at the decision time `at`, which the coordinator selected and retained, so a
    /// delayed delivery or a restart reproduces the same acceptance and expiry times.
    /// # Errors
    /// `NotAvailable` for an unknown key or a token that is not the prepared one, `Conflict` for an aborted operation, otherwise the failure
    /// of applying (the effect stays prepared and committing again is safe), or `Storage` when the journal write fails after the effect was
    /// applied (committing again completes it).
    pub fn commit_at(
        &self,
        key: OperationKey,
        token: &[u8; 32],
        at: u64,
    ) -> Result<Outcome, Error> {
        tracing::trace!("Committing a hosted publication effect at a retained decision time");
        match self.journal.get(key) {
            None => Err(Error::NotAvailable),
            Some(Entry::Aborted) => Err(Error::Conflict),
            Some(Entry::Committed(outcome)) => Ok(outcome),
            Some(Entry::Prepared {
                token: expected, ..
            }) if expected != *token => Err(Error::NotAvailable),
            Some(Entry::Prepared { effect, .. }) => {
                let outcome = self.apply(key, &effect, at)?;
                self.journal.put(key, Entry::Committed(outcome.clone()))?;
                self.release(&effect);
                Ok(outcome)
            }
        }
    }

    /// Release a prepared effect that the coordinator aborted. Aborting an absent or already aborted operation is a no-op.
    /// # Errors
    /// `Conflict` for a committed operation, `NotAvailable` for a token that is not the prepared one, `Storage` on a failed write.
    pub fn abort(&self, key: OperationKey, token: Option<&[u8; 32]>) -> Result<(), Error> {
        tracing::trace!("Aborting a hosted publication effect");
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
                self.release(&effect);
                Ok(())
            }
        }
    }

    /// The durable state of an operation.
    pub fn status(&self, key: OperationKey) -> Status {
        tracing::trace!("Reading a hosted publication participant status");
        match self.journal.get(key) {
            None => Status::Absent,
            Some(Entry::Prepared { token, .. }) => Status::Prepared(token),
            Some(Entry::Committed(outcome)) => Status::Committed(Box::new(outcome)),
            Some(Entry::Aborted) => Status::Aborted,
        }
    }

    /// Every prepared operation, for recovery.
    pub fn prepared(&self) -> Vec<(OperationKey, [u8; 32])> {
        tracing::trace!("Listing prepared hosted publication operations");
        self.journal
            .prepared()
            .into_iter()
            .map(|(key, _, token)| (key, token))
            .collect()
    }

    /// Record a read-only operation as prepared and committed under `key`, so its retained request survives a restart and its (empty)
    /// outcome is answered from the journal. Returns the token.
    /// # Errors
    /// As for [`Participant::prepare`].
    pub fn stage_query(&self, key: OperationKey, request: Vec<u8>) -> Result<[u8; 32], Error> {
        tracing::trace!("Staging a read-only hosted publication operation");
        self.prepare(key, &Effect::Query { request })
    }

    // Check, without changing anything, that the effect could be applied now.
    fn check_preconditions(&self, effect: &Effect, now: u64) -> Result<(), Error> {
        let store = self.provider.store();
        match effect {
            Effect::Query { .. } => Ok(()),
            Effect::Store {
                placement_id,
                publication,
                ..
            } => {
                if store.record(*placement_id).is_some()
                    || store.conflicts(&publication.metadata, now)
                {
                    return Err(Error::Conflict);
                }
                if store.has_room(publication.content.len() as u64) {
                    Ok(())
                } else {
                    Err(Error::Limit)
                }
            }
            Effect::Renew {
                placement_id,
                expected_expires_at,
                ..
            } => {
                let record = lifecycle::touch(store, *placement_id, now)?;
                if !record.active_at(now) {
                    return Err(Error::NotAvailable);
                }
                if record.expires_at != *expected_expires_at {
                    Err(Error::Conflict)
                } else {
                    Ok(())
                }
            }
            Effect::Remove { request, .. } => {
                let record = lifecycle::touch(store, request.placement_id, now)?;
                if !record.active_at(now) {
                    return Err(Error::NotAvailable);
                }
                if record.expires_at != request.expected_expires_at {
                    Err(Error::Conflict)
                } else {
                    Ok(())
                }
            }
        }
    }

    // Apply an effect idempotently through the provider engine, whose commits recognize an effect already visible in the store.
    fn apply(&self, key: OperationKey, effect: &Effect, at: u64) -> Result<Outcome, Error> {
        match effect {
            Effect::Query { .. } => Ok(Outcome::Query),
            Effect::Store {
                placement_id,
                publication,
                retention_seconds,
                price,
            } => {
                let plan = StorePlan {
                    publisher: publication.metadata.publisher_id,
                    publication: (**publication).clone(),
                    retention_seconds: *retention_seconds,
                    price: *price,
                };
                let output = self.provider.commit_store(&plan, *placement_id, at)?;
                Ok(Outcome::Stored {
                    placement_id: output.placement_id,
                    publication_id: output.publication_id,
                    accepted_at: output.accepted_at,
                    expires_at: output.expires_at,
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
                let plan = RenewPlan {
                    placement_id: *placement_id,
                    expected_expires_at: *expected_expires_at,
                    additional_seconds: *additional_seconds,
                    ceiling_expires_at: *ceiling_expires_at,
                    price: *price,
                };
                let (output, price) = self.provider.commit_renew(&plan, at)?;
                Ok(Outcome::Renewed {
                    placement_id: output.placement_id,
                    previous_expires_at: output.previous_expires_at,
                    expires_at: output.expires_at,
                    price,
                })
            }
            Effect::Remove {
                request,
                release_credits,
                price,
            } => {
                let quote = RemovalQuote {
                    release_credits: *release_credits,
                    price: *price,
                };
                let done = self.provider.commit_remove(key, request, quote, at)?;
                Ok(Outcome::Removed {
                    placement_id: done.placement_id,
                    removal_id: done.removal_id,
                    release_credits: done.release_credits,
                    price: done.price,
                })
            }
        }
    }

    // Free everything an effect held.
    fn release(&self, effect: &Effect) {
        let mut locks = self.locks();
        for lock in lock_set(effect) {
            locks.remove(&lock);
        }
    }

    fn locks(&self) -> std::sync::MutexGuard<'_, BTreeMap<Lock, OperationKey>> {
        self.locks.lock().unwrap_or_else(|e| e.into_inner())
    }
}

// Everything an effect holds exclusively: its placement, and for a store its publication ID and page or feed sequence slot.
fn lock_set(effect: &Effect) -> Vec<Lock> {
    match effect {
        Effect::Query { .. } => vec![],
        Effect::Store {
            placement_id,
            publication,
            ..
        } => {
            let m = &publication.metadata;
            let mut locks = vec![
                Lock::Placement(*placement_id),
                Lock::Publication(m.publisher_id, m.publication_id),
            ];
            if m.kind.is_listed() {
                locks.push(Lock::Sequence(m.publisher_id, m.kind.code(), m.sequence));
            }
            locks
        }
        Effect::Renew { placement_id, .. } => vec![Lock::Placement(*placement_id)],
        Effect::Remove { request, .. } => vec![Lock::Placement(request.placement_id)],
    }
}

#[cfg(test)]
#[path = "participant_tests.rs"]
mod tests;
