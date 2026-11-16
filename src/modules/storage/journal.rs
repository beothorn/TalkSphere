//! Durable Storage participant staging and preparation; no remote retries or coordinator decisions are inferred.
use super::{
    contract::{Crypto, Persistence},
    image::{Image, Limits, Retained, Stage},
    retention::Anchor,
};
use crate::modules::{
    identity::api::IdentityId,
    services::{
        Error,
        contract::OperationKey,
        handler::{Blob, Completion},
    },
};
use std::sync::{Arc, Mutex};

/// Serialized family-owned journal, gated after an uncertain snapshot write until explicit reload.
pub struct Journal {
    provider: IdentityId,
    persistence: Arc<dyn Persistence>,
    crypto: Arc<dyn Crypto>,
    limits: Limits,
    state: Mutex<Option<Image>>,
}
impl Journal {
    /// Explicitly initialize an absent owner journal; existing metadata is never overwritten as fresh state.
    /// The platform adapter must already hold exclusive owner-writer authority.
    /// # Errors
    /// Conflict rejects existing state; dependency and uncertain commit failures preserve their exact meaning.
    pub fn initialize(
        provider: IdentityId,
        persistence: Arc<dyn Persistence>,
        crypto: Arc<dyn Crypto>,
        limits: Limits,
        anchor: Anchor,
    ) -> Result<Self, Error> {
        tracing::trace!("Initializing an explicitly authorized Storage participant journal");
        if persistence.load(limits.bytes)?.is_some() {
            return Err(Error::Conflict);
        }
        let image = Image {
            provider,
            anchor,
            leases: vec![],
            operations: vec![],
        };
        persistence.replace(&image.encode(limits)?)?;
        Ok(Self {
            provider,
            persistence,
            crypto,
            limits,
            state: Mutex::new(Some(image)),
        })
    }
    /// Reopen only existing complete metadata, without treating corruption or absence as fresh state.
    /// Byte integrity and coordinator recovery must complete before a provider advertises new work.
    /// # Errors
    /// Unavailable rejects absent state; image and persistence failures propagate before admission.
    pub fn open(
        provider: IdentityId,
        persistence: Arc<dyn Persistence>,
        crypto: Arc<dyn Crypto>,
        limits: Limits,
    ) -> Result<Self, Error> {
        tracing::trace!("Reopening existing Storage participant metadata");
        let bytes = persistence.load(limits.bytes)?.ok_or(Error::Unavailable)?;
        let image = Image::decode(&bytes, provider, limits)?;
        Ok(Self {
            provider,
            persistence,
            crypto,
            limits,
            state: Mutex::new(Some(image)),
        })
    }
    /// Retain an original-key complete proposed effect/result durably before returning staged success.
    /// Exact local repeats recover the same completion; changed key reuse conflicts without execution.
    /// Callers are trusted local family/Services adapters, not peer payload handlers.
    /// # Errors
    /// Conflict rejects changed reuse, Limit exhausted image budgets and Unknown possible snapshot commitment.
    pub fn stage(
        &self,
        key: OperationKey,
        binding: [u8; 32],
        completion: Completion,
    ) -> Result<Completion, Error> {
        tracing::trace!("Durably retaining original Storage work and result");
        completion.output.bounded(self.limits.values)?;
        if completion.result.bytes().len() > self.limits.bytes
            || completion.effect.bytes().len() > self.limits.bytes
        {
            return Err(Error::Limit);
        }
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        let image = state.as_ref().ok_or(Error::Unknown)?;
        if let Some(existing) = image.operations.iter().find(|r| r.key == key) {
            if existing.binding != binding || !same_completion(&existing.completion, &completion) {
                return Err(Error::Conflict);
            }
            if matches!(existing.status, Stage::Aborted(_)) {
                return Err(Error::Conflict);
            }
            return Ok(existing.completion.clone());
        }
        let mut candidate = image.clone();
        candidate.operations.push(Retained {
            key,
            binding,
            completion: completion.clone(),
            status: Stage::Staged,
        });
        self.persist(&mut state, candidate)?;
        Ok(completion)
    }
    /// Return retained complete output without repeating execution or making a new reservation.
    /// # Errors
    /// Unavailable rejects absent operations; Conflict rejects aborted work; Unknown gates uncertain metadata.
    pub fn completion(&self, key: OperationKey) -> Result<Completion, Error> {
        tracing::trace!("Recovering original Storage completion without execution");
        let state = self.state.lock().map_err(|_| Error::Unavailable)?;
        let image = state.as_ref().ok_or(Error::Unknown)?;
        let record = image
            .operations
            .iter()
            .find(|r| r.key == key)
            .ok_or(Error::Unavailable)?;
        if matches!(record.status, Stage::Aborted(_)) {
            return Err(Error::Conflict);
        }
        Ok(record.completion.clone())
    }
    /// Prepare only the exact staged effect with a durable owner/key/binding/effect-bound opaque token.
    /// Repeated preparation returns the original token, including after committed response loss.
    /// # Errors
    /// Conflict rejects mismatched effects or aborted work; absent records are Unavailable; dependency failures propagate.
    pub fn prepare(&self, key: OperationKey, effect: &Blob) -> Result<Blob, Error> {
        tracing::trace!("Preparing exact original Storage effect");
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        let image = state.as_ref().ok_or(Error::Unknown)?;
        let index = image
            .operations
            .iter()
            .position(|r| r.key == key)
            .ok_or(Error::Unavailable)?;
        let record = &image.operations[index];
        if &record.completion.effect != effect {
            return Err(Error::Conflict);
        }
        match &record.status {
            Stage::Prepared(token) | Stage::Committed(token) => return Ok(token.clone()),
            Stage::Aborted(_) => return Err(Error::Conflict),
            Stage::Staged => {}
        }
        let nonce = self.crypto.lease_id()?;
        let mut binding = b"TalkSphere Storage participant token v1".to_vec();
        for bytes in [
            &self.provider.0[..],
            &key.requester.0,
            &key.connection,
            &key.stream.to_be_bytes(),
            &record.binding,
            &nonce,
            effect.bytes(),
        ] {
            binding.extend_from_slice(bytes);
        }
        let digest = self.crypto.hash(&binding)?;
        let mut token = nonce.to_vec();
        token.extend_from_slice(&digest);
        let token = Blob::new(token, 64)?;
        let mut candidate = image.clone();
        candidate.operations[index].status = Stage::Prepared(token.clone());
        self.persist(&mut state, candidate)?;
        Ok(token)
    }
    /// Observe family-local staged/prepared/terminal state without converting timeout into abort.
    /// # Errors
    /// Unknown gates possibly committed metadata; poisoned locks are Unavailable.
    pub fn status(&self, key: OperationKey) -> Result<Option<Stage>, Error> {
        tracing::trace!("Observing durable Storage participant phase");
        let state = self.state.lock().map_err(|_| Error::Unavailable)?;
        let image = state.as_ref().ok_or(Error::Unknown)?;
        Ok(image
            .operations
            .iter()
            .find(|r| r.key == key)
            .map(|r| r.status.clone()))
    }
    /// Reload installed status after reply loss, never executing work or choosing a coordinator decision.
    /// # Errors
    /// Missing acknowledged metadata is Unknown; corruption/identity/persistence failures retain the gate.
    pub fn reload(&self) -> Result<(), Error> {
        tracing::trace!("Reloading Storage participant status after uncertain persistence");
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        *state = None;
        let bytes = self
            .persistence
            .load(self.limits.bytes)?
            .ok_or(Error::Unknown)?;
        let image = Image::decode(&bytes, self.provider, self.limits)?;
        *state = Some(image);
        Ok(())
    }
    // Install only a completely encoded acknowledged candidate; possible commitment closes the owner gate.
    fn persist(&self, state: &mut Option<Image>, candidate: Image) -> Result<(), Error> {
        tracing::trace!("Acknowledging an atomic Storage participant image");
        let bytes = candidate.encode(self.limits)?;
        match self.persistence.replace(&bytes) {
            Ok(()) => {
                *state = Some(candidate);
                Ok(())
            }
            Err(Error::Unknown) => {
                *state = None;
                Err(Error::Unknown)
            }
            Err(error) => Err(error),
        }
    }
}
// Exact completion equality prevents a same-key local retry from changing output, effect or charge.
fn same_completion(a: &Completion, b: &Completion) -> bool {
    tracing::trace!("Checking immutable original Storage completion binding");
    a.output == b.output && a.result == b.result && a.effect == b.effect && a.price == b.price
}

#[cfg(test)]
#[path = "tests/journal.rs"]
mod tests;
