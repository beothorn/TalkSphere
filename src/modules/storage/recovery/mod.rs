//! Linux Storage startup admission over public coordinator recovery and independently durable family state.
mod cleanup;
mod usage;
use super::{
    bytes::Store,
    contract::{Limits, Time},
    image::Stage,
    journal::Journal,
    quota::Quota,
};
use crate::modules::{
    identity::api::IdentityId,
    services::{
        Error,
        contract::{Context, OperationKey},
        persistence::Persistence,
        runtime::{Coordinator, Phase},
    },
};
pub use cleanup::Cleanup;
use std::sync::{Arc, Mutex};

/// Trusted local coordinator recovery; it resumes original records rather than executing work again.
pub trait Resume {
    /// Identify the coordinator's authoritative local owner.
    fn local_identity(&self) -> IdentityId;
    /// Resume one original operation through the coordinator's public authorized recovery contract.
    /// # Errors
    /// Missing, unavailable or uncertain coordination must fail without implying abort.
    fn resume(&self, key: OperationKey) -> Result<Phase, Error>;
    /// Enumerate original unresolved local Storage provider operations.
    /// # Errors
    /// Unavailable coordinator state prevents new admission.
    fn pending_storage(&self) -> Result<Vec<OperationKey>, Error> {
        tracing::trace!("Using an empty pending-work inventory for an isolated Storage owner");
        Ok(vec![])
    }

    /// Recover an unresolved Services entry without an installed Storage record.
    /// # Errors
    /// Absence of a family record cannot undo a recorded commit.
    fn recover_absent(&self, _key: OperationKey) -> Result<Phase, Error> {
        tracing::trace!("Refusing unowned coordinator-only Storage recovery");
        Err(Error::Unknown)
    }

    /// Resolve an original staged upload whose exact immutable object was never installed.
    /// # Errors
    /// Missing authority or any potentially committed decision fails without cleanup.
    fn abort_unfinished_upload(&self, _key: OperationKey) -> Result<Phase, Error> {
        tracing::trace!("Refusing unavailable Storage upload abort authority");
        Err(Error::Unknown)
    }
}
/// Direct public Services recovery adapter; caller grants/deadlines remain enforced by Services.
pub struct Services<'a, P: Persistence> {
    /// Existing coordinator whose registry retains accepted Storage handlers.
    pub coordinator: &'a Coordinator<P>,
    /// Authorized local recovery/coordinate context, independent of peer input.
    pub context: &'a Context<'a>,
}
impl<P: Persistence> Resume for Services<'_, P> {
    // Read only the public owner identity, never another module's private records.
    fn local_identity(&self) -> IdentityId {
        tracing::trace!("Reading Storage recovery coordinator identity");
        self.coordinator.local_identity()
    }
    // Services alone resolves its original phases and delivers recorded participant decisions.
    fn resume(&self, key: OperationKey) -> Result<Phase, Error> {
        tracing::trace!("Resuming original Storage coordination through Services");
        self.coordinator.resume(self.context, key)
    }
    // Only this local coordinator can enumerate its unresolved provider work.
    fn pending_storage(&self) -> Result<Vec<OperationKey>, Error> {
        tracing::trace!("Enumerating original pending Storage work through Services");
        self.coordinator
            .pending_provider(self.context, "cr2se.storage", 1)
    }

    // Confirm an absent Storage participant from the recovered owner before reconciling its Services record.
    fn recover_absent(&self, key: OperationKey) -> Result<Phase, Error> {
        tracing::trace!(
            "Reconciling original requester reservations for Storage without a family effect"
        );
        self.coordinator
            .recover_absent_provider(self.context, key, "cr2se.storage", 1)
    }

    // Make one explicit local decision through Services, which refuses any contradictory persisted choice.
    fn abort_unfinished_upload(&self, key: OperationKey) -> Result<Phase, Error> {
        tracing::trace!("Resolving a known uninstalled original Storage upload");
        self.coordinator.decide(
            self.context,
            key,
            crate::modules::services::runtime::Decision::Abort,
        )?;
        self.coordinator.abort(self.context, key)?;
        Ok(Phase::Aborted)
    }
}
struct Admission {
    ready: bool,
    quota: Option<Quota>,
}
/// Closed-until-recovered admission owner; lifecycle recovery remains callable while new work is blocked.
pub struct Gate {
    journal: Arc<Journal>,
    bytes: Arc<Store>,
    limits: Limits,
    admission: Mutex<Admission>,
    recovery: Mutex<()>,
}
impl Gate {
    /// Construct a closed owner with finite policy; neither constructor nor reopen advertises availability.
    /// # Errors
    /// Invalid rejects unusable capacity policy before new work.
    pub fn new(journal: Arc<Journal>, bytes: Arc<Store>, limits: Limits) -> Result<Self, Error> {
        tracing::trace!("Closing Storage new-work admission pending recovery");
        limits.validate()?;
        Ok(Self {
            journal,
            bytes,
            limits,
            admission: Mutex::new(Admission {
                ready: false,
                quota: None,
            }),
            recovery: Mutex::new(()),
        })
    }
    /// Resolve original participants, recover time/bytes and reconstruct every retained hold before admitting work.
    /// `reboot_elapsed` is independently trusted elapsed time since the acknowledged anchor across boot changes.
    /// No absent decision, timeout or failed dependency releases a hold or creates another peer invocation.
    /// # Errors
    /// Denied rejects a foreign coordinator; Unknown rejects unresolved/inconsistent decisions;
    /// storage, clock, corruption and quota failures leave admission closed.
    pub fn recover(
        &self,
        coordinator: &dyn Resume,
        now: Time,
        reboot_elapsed: Option<u64>,
    ) -> Result<(), Error> {
        tracing::trace!("Recovering Storage obligations before new-work admission");
        let _recovery = self.recovery.lock().map_err(|_| Error::Unavailable)?;
        {
            let mut admission = self.admission.lock().map_err(|_| Error::Unavailable)?;
            admission.ready = false;
        }
        self.journal.reload()?;
        let image = self.journal.snapshot()?;
        if coordinator.local_identity() != image.provider {
            return Err(Error::Denied);
        }
        let limits = self.journal.image_limits();
        let maximum = limits
            .leases
            .checked_add(limits.operations)
            .ok_or(Error::Limit)?;
        let inventory = self.bytes.inventory(maximum)?;
        for record in &image.operations {
            let effect = super::effect::Effect::decode(
                record.completion.effect.bytes(),
                image.provider,
                limits,
            )?;
            let unfinished = matches!(record.status, Stage::Staged)
                && matches!(effect, super::effect::Effect::CreateTemplate(ref lease)
                    if !inventory.iter().any(|entry| entry.id == lease.id && !entry.pending));
            let phase = if unfinished {
                coordinator.abort_unfinished_upload(record.key)?
            } else {
                coordinator.resume(record.key)?
            };
            let status = self.journal.status(record.key)?;
            if phase == Phase::Aborted
                && let super::effect::Effect::CreateTemplate(lease) = effect
            {
                self.discard_aborted_upload(lease.id)?;
            }
            if !matches!(
                (phase, status),
                (Phase::Committed, Some(Stage::Committed(_)))
                    | (Phase::Aborted, Some(Stage::Aborted(_)))
            ) {
                return Err(Error::Unknown);
            }
        }
        for key in coordinator.pending_storage()? {
            if self.journal.status(key)?.is_some()
                || !matches!(
                    coordinator.recover_absent(key)?,
                    Phase::Committed | Phase::Aborted
                )
            {
                return Err(Error::Unknown);
            }
        }
        let image = self.journal.recovered_time(now, reboot_elapsed)?;
        image.verify_active(&self.bytes)?;
        let maximum = self
            .journal
            .image_limits()
            .leases
            .checked_add(self.journal.image_limits().operations)
            .ok_or(Error::Limit)?;
        let inventory = self.bytes.inventory(maximum)?;
        for entry in inventory {
            if entry.pending {
                return Err(Error::Unavailable);
            }
            let mut known = image.leases.iter().any(|l| l.id == entry.id);
            for record in &image.operations {
                if let Some(proposal) = super::effect::Effect::decode(
                    record.completion.effect.bytes(),
                    image.provider,
                    self.journal.image_limits(),
                )?
                .created_lease()
                {
                    known |= proposal.id == entry.id;
                }
            }
            if !known {
                return Err(Error::Unavailable);
            }
        }

        let retained = usage::objects(&self.bytes, &image, self.journal.image_limits())?;
        let quota = usage::rebuild(&image, self.journal.image_limits(), self.limits, &retained)?;
        let mut admission = self.admission.lock().map_err(|_| Error::Unavailable)?;
        admission.quota = Some(quota);
        admission.ready = true;
        Ok(())
    }
    /// Observe whether new-work admission has passed complete recovery.
    /// # Errors
    /// A poisoned admission lock is Unavailable.
    pub fn is_ready(&self) -> Result<bool, Error> {
        tracing::trace!("Observing Storage recovery admission state");
        Ok(self.admission.lock().map_err(|_| Error::Unavailable)?.ready)
    }
    /// Run one trusted bounded new-work admission action while the recovery gate and quota view remain stable.
    /// The action must reserve capacity before platform writes and explicitly retain holds through uncertainty.
    /// It must not reenter this gate or wait for network I/O. Lifecycle decisions use the separate ports below.
    /// # Errors
    /// Unavailable rejects closed admission; action failures propagate, and Unknown closes admission for recovery.
    pub fn admit<T>(
        &self,
        action: impl FnOnce(&Journal, &Quota) -> Result<T, Error>,
    ) -> Result<T, Error> {
        tracing::trace!("Admitting bounded Storage work against recovered obligations");
        let mut admission = self.admission.lock().map_err(|_| Error::Unavailable)?;
        if !admission.ready {
            return Err(Error::Unavailable);
        }
        let result = action(
            &self.journal,
            admission.quota.as_ref().ok_or(Error::Unknown)?,
        );
        if matches!(result, Err(Error::Unknown)) {
            admission.ready = false;
        }
        result
    }
    /// Observe original participant phase through the recovery boundary, without admitting new work.
    /// # Errors
    /// Preserves exact journal unavailable/unknown outcomes.
    pub fn status(&self, key: OperationKey) -> Result<Option<Stage>, Error> {
        tracing::trace!("Observing Storage participant through recovery admission");
        self.journal.status(key)
    }
    /// Recover retained original completion while new-work admission is closed.
    /// # Errors
    /// Journal authority/status failures retain their typed meaning; this never executes work.
    pub fn completion(
        &self,
        key: OperationKey,
    ) -> Result<crate::modules::services::handler::Completion, Error> {
        tracing::trace!("Recovering Storage completion through closed admission");
        self.journal.completion(key)
    }
    /// Deliver preparation during authorized original-key recovery without admitting new work.
    /// # Errors
    /// Propagates exact journal preparation failures.
    pub fn prepare(
        &self,
        key: OperationKey,
        effect: &crate::modules::services::handler::Blob,
    ) -> Result<crate::modules::services::handler::Blob, Error> {
        tracing::trace!("Delivering Storage recovery preparation");
        let (image, limits) = self.snapshot()?;
        if let super::effect::Effect::CreateTemplate(lease) =
            super::effect::Effect::decode(effect.bytes(), image.provider, limits)?
            && !matches!(self.journal.status(key)?, Some(Stage::Committed(_)))
        {
            self.bytes.reopen(lease.id, lease.size_bytes)?;
        }
        self.observe(self.journal.prepare(key, effect))
    }
    /// Deliver the coordinator's immutable commit while new work remains gated.
    /// # Errors
    /// Propagates contradictory tokens, unavailable resources and uncertain local commitment.
    pub fn commit(
        &self,
        key: OperationKey,
        token: &crate::modules::services::handler::Blob,
    ) -> Result<(), Error> {
        tracing::trace!("Delivering Storage recovery commit");
        self.terminal(key, self.journal.commit(key, token))
    }
    /// Apply a finalized store decision and update the original capacity hold.
    /// # Errors
    /// Preserves conflicting decisions, malformed finalization and durability uncertainty.
    pub fn commit_store(
        &self,
        key: OperationKey,
        token: &crate::modules::services::handler::Blob,
        template: &crate::modules::services::handler::Completion,
        finalized: &crate::modules::services::handler::Completion,
    ) -> Result<(), Error> {
        tracing::trace!("Delivering finalized Storage creation through recovery gate");
        self.terminal(
            key,
            self.journal.commit_store(key, token, template, finalized),
        )
    }

    // Read the original immutable metadata and its limits for decision selection.
    pub(super) fn snapshot(&self) -> Result<(super::image::Image, super::image::Limits), Error> {
        tracing::trace!("Reading Storage state for the original operation");
        Ok((self.journal.snapshot()?, self.journal.image_limits()))
    }

    // Share the protected byte owner inside Storage without granting direct access to other modules.
    pub(super) fn byte_store(&self) -> &Store {
        tracing::trace!("Accessing original Storage byte owner");
        &self.bytes
    }

    /// Deliver only the coordinator's immutable abort while preserving original preparation tokens.
    /// # Errors
    /// Propagates contradictory decisions and uncertainty without releasing another owner's resources.
    pub fn abort(
        &self,
        key: OperationKey,
        token: Option<&crate::modules::services::handler::Blob>,
    ) -> Result<(), Error> {
        tracing::trace!("Delivering Storage recovery abort");
        self.observe(self.journal.abort(key, token))?;
        let cleanup: Result<(), Error> = (|| {
            let image = self.journal.snapshot()?;
            let record = image
                .operations
                .iter()
                .find(|r| r.key == key)
                .ok_or(Error::Unknown)?;
            let effect = super::effect::Effect::decode(
                record.completion.effect.bytes(),
                image.provider,
                self.journal.image_limits(),
            )?;
            if let super::effect::Effect::CreateTemplate(lease) = effect {
                self.discard_aborted_upload(lease.id)?;
            }
            Ok(())
        })();
        self.terminal(key, cleanup.map_err(|_| Error::Unknown))
    }
    // Reconcile both paths after a known abort, including lost physical cleanup acknowledgements.
    fn discard_aborted_upload(&self, id: [u8; 32]) -> Result<(), Error> {
        tracing::trace!("Discarding byte resources for a durably aborted Storage upload");
        self.bytes.discard_upload(id)?;
        if !self.bytes.reclaim(id)? {
            return Err(Error::Unknown);
        }
        Ok(())
    }

    // Close admission immediately when any lifecycle write may have installed an unacknowledged image.
    fn observe<T>(&self, result: Result<T, Error>) -> Result<T, Error> {
        tracing::trace!("Preserving Storage admission gate across participant uncertainty");
        if matches!(result, Err(Error::Unknown)) {
            self.admission.lock().map_err(|_| Error::Unknown)?.ready = false;
        }
        result
    }
    // Release only known terminal pending-work capacity; committed byte/history/result obligations remain held.
    fn terminal(&self, key: OperationKey, result: Result<(), Error>) -> Result<(), Error> {
        tracing::trace!("Updating recovered Storage admission after an acknowledged decision");
        self.observe(result)?;
        let mut admission = self.admission.lock().map_err(|_| Error::Unknown)?;
        if !admission.ready {
            return Ok(());
        }
        let update = (|| {
            let image = self.journal.snapshot()?;
            let record = image
                .operations
                .iter()
                .find(|r| r.key == key)
                .ok_or(Error::Unknown)?;
            let retained = usage::objects(&self.bytes, &image, self.journal.image_limits())?;
            let usage =
                usage::record_usage(&image, record, self.journal.image_limits(), &retained)?;
            admission
                .quota
                .as_ref()
                .ok_or(Error::Unknown)?
                .transition(key, usage)
        })();
        if update.is_err() {
            admission.ready = false;
            return Err(Error::Unknown);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/recovery.rs"]
mod tests;
