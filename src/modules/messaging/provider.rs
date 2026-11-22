//! The Messaging provider engine: assessing and committing stores.
//!
//! Callers: the Services provider adapter and standalone tests. A store is assessed from its header alone,
//! before any payload body is received: the sender must be the authenticated caller and derive from the
//! supplied key, sizes, encoding and retention must fit the offering, the exact price is calculated and a
//! bounded staging reservation is taken. When the payload has arrived it is verified against the header (the
//! signature covers the payload, so it can only be checked now) and the effect is prepared and committed
//! through the participant, which assigns the acceptance time at the durable commit. Every failure before
//! commitment is uncharged and leaves nothing reserved or recorded.
use super::{
    effect::{Effect, Outcome},
    error::Error,
    ids::{PlacementId, Signature},
    metadata::{Metadata, SignedMessage},
    operation::{
        CheckInput, CheckOutput, DiscoverInput, DiscoverOutput, Entry, RecoverInput, RecoverOutput,
        RemoveInput, RemoveOutput, RenewInput, RenewOutput, StatusInput, StatusOutput, StoreInput,
        StoreOutput,
    },
    participant::Participant,
    ports::{Clock, FixedLimits, NoRelease, PendingLimits, ReleasePolicy},
    pricing, receipt, signing,
    staging::{Reservation, Staging},
    state::{Encoding, MINIMUM_ENCRYPTED_BYTES, State},
    terms::Terms,
};
use crate::modules::{
    cryptography::credential, identity::api::IdentityId, services::contract::OperationKey,
};
use std::sync::Arc;

/// Everything about a store except the payload bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreHeader {
    /// Signed metadata including the declared payload size and hash.
    pub metadata: Metadata,
    /// The sender's public key.
    pub sender_public_key: [u8; 32],
    /// The sender's signature.
    pub sender_signature: Signature,
    /// Requested initial retention.
    pub retention_seconds: u64,
}

impl StoreInput {
    /// The header of this store, without the payload.
    pub fn header(&self) -> StoreHeader {
        StoreHeader {
            metadata: self.message.metadata.clone(),
            sender_public_key: self.message.sender_public_key,
            sender_signature: self.message.sender_signature,
            retention_seconds: self.retention_seconds,
        }
    }
}

/// A store accepted for assessment: its exact price and its held staging reservation.
pub struct StoreAssessment {
    /// The authenticated caller the store was assessed for.
    pub caller: IdentityId,
    /// The exact price to be charged on success.
    pub price: u64,
    header: StoreHeader,
    _reservation: Reservation,
}

/// A store prepared durably and awaiting the coordinator's decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedStore {
    /// The Services operation key.
    pub key: OperationKey,
    /// The commit token.
    pub token: [u8; 32],
    /// The placement that commit will create.
    pub placement_id: PlacementId,
    /// The exact price.
    pub price: u64,
}

/// The provider engine.
pub struct Provider {
    terms: Terms,
    participant: Arc<Participant>,
    clock: Arc<dyn Clock>,
    staging: Staging,
    release: Arc<dyn ReleasePolicy>,
    limits: Arc<dyn PendingLimits>,
}

impl Provider {
    /// A provider for validated terms over a participant.
    pub fn new(
        terms: Terms,
        participant: Arc<Participant>,
        clock: Arc<dyn Clock>,
        staging: Staging,
    ) -> Self {
        tracing::trace!("Creating the Messaging provider");
        Self {
            terms,
            participant,
            clock,
            staging,
            release: Arc::new(NoRelease),
            limits: Arc::new(FixedLimits::default()),
        }
    }

    /// Bound pending placements per sender and per recipient with `limits`; the default is 1024 each.
    #[must_use]
    pub fn with_pending_limits(mut self, limits: Arc<dyn PendingLimits>) -> Self {
        tracing::trace!("Installing the pending placement limits");
        self.limits = limits;
        self
    }

    /// Use `policy` to choose early-release credits when removals are quoted; without it none are offered.
    #[must_use]
    pub fn with_release_policy(mut self, policy: Arc<dyn ReleasePolicy>) -> Self {
        tracing::trace!("Installing the release credit policy");
        self.release = policy;
        self
    }

    /// The terms this provider serves.
    pub fn terms(&self) -> &Terms {
        &self.terms
    }

    /// The participant.
    pub fn participant(&self) -> &Arc<Participant> {
        &self.participant
    }

    /// Assess a store from its header, before the payload body is received.
    /// # Errors
    /// `Invalid` when the sender is not the authenticated caller or does not derive from the key, a size, encoding or retention is outside
    /// the offering or the expiry would overflow; `Limit` when the sender or the recipient already holds the most pending placements allowed, the staging budget is full or a price would overflow. Nothing is reserved on failure.
    pub fn assess_store(
        &self,
        caller: IdentityId,
        header: StoreHeader,
    ) -> Result<StoreAssessment, Error> {
        tracing::trace!("Assessing a store before its payload arrives");
        let m = &header.metadata;
        if m.sender_id != caller {
            return Err(Error::Invalid);
        }
        let derived =
            credential::identity_id(&header.sender_public_key).map_err(|_| Error::Invalid)?;
        if derived != m.sender_id.0 {
            return Err(Error::Invalid);
        }
        let t = &self.terms;
        if m.payload_size == 0 || m.payload_size > t.maximum_payload_bytes {
            return Err(Error::Invalid);
        }
        if m.encoding == Encoding::IdentityEncryptedV1
            && m.payload_size < MINIMUM_ENCRYPTED_BYTES as u64
        {
            return Err(Error::Invalid);
        }
        let retention = header.retention_seconds;
        if retention < t.minimum_retention_seconds || retention > t.maximum_retention_seconds {
            return Err(Error::Invalid);
        }
        let now = self.clock.now()?;
        now.checked_add(retention).ok_or(Error::Invalid)?;
        // Per-identity bounds are judged before any body is accepted or any price is quoted, so a refusal costs nothing.
        let (by_sender, for_recipient) = self
            .participant
            .store()
            .pending_counts(m.sender_id, m.recipient_id);
        if by_sender >= self.limits.per_sender(m.sender_id)
            || for_recipient >= self.limits.per_recipient(m.recipient_id)
        {
            return Err(Error::Limit);
        }
        let price = pricing::store(
            m.payload_size,
            retention,
            t.payload_usage,
            t.payload_storage,
            t.minimum_price,
        )?;
        let reservation = self.staging.reserve(m.payload_size)?;
        Ok(StoreAssessment {
            caller,
            price,
            header,
            _reservation: reservation,
        })
    }

    /// Verify the received payload against the assessed header and prepare the store durably.
    /// # Errors
    /// `Invalid` when the message differs from the assessed header or fails verification (size, hash, signature); otherwise the participant's refusals.
    /// The reservation is released when the assessment is dropped.
    pub fn prepare_store(
        &self,
        key: OperationKey,
        assessment: &StoreAssessment,
        message: SignedMessage,
    ) -> Result<PreparedStore, Error> {
        tracing::trace!("Preparing a store after its payload arrived");
        let effect = self.plan_store(assessment, message)?;
        let Effect::Store {
            placement_id,
            price,
            ..
        } = &effect
        else {
            return Err(Error::Conflict);
        };
        let (placement_id, price) = (*placement_id, *price);
        let token = self.participant.prepare(key, &effect)?;
        Ok(PreparedStore {
            key,
            token,
            placement_id,
            price,
        })
    }

    /// Verify the received payload against the assessed header and return the store effect to prepare, with a fresh placement ID.
    /// Nothing is recorded.
    /// # Errors
    /// `Invalid` when the message differs from the assessed header or fails verification; `Unavailable` without randomness.
    pub fn plan_store(
        &self,
        assessment: &StoreAssessment,
        message: SignedMessage,
    ) -> Result<Effect, Error> {
        tracing::trace!("Planning a store after its payload arrived");
        let expected = &assessment.header;
        if message.metadata != expected.metadata
            || message.sender_public_key != expected.sender_public_key
            || message.sender_signature != expected.sender_signature
        {
            return Err(Error::Invalid);
        }
        signing::verify_message(&message, Some(assessment.caller))?;
        let placement_id = self.participant.new_placement_id()?;
        Ok(Effect::Store {
            placement_id,
            message,
            retention_seconds: expected.retention_seconds,
            price: assessment.price,
        })
    }

    /// Commit a prepared store and return its result.
    /// # Errors
    /// The participant's commit errors; the store stays prepared and committing again is safe.
    pub fn commit_store(&self, prepared: &PreparedStore) -> Result<StoreOutput, Error> {
        tracing::trace!("Committing a prepared store");
        match self.participant.commit(prepared.key, &prepared.token)? {
            Outcome::Stored {
                placement_id,
                message_id,
                accepted_at,
                expires_at,
                price,
            } => Ok(StoreOutput {
                placement_id,
                message_id,
                accepted_at,
                expires_at,
                price,
            }),
            _ => Err(Error::Conflict),
        }
    }

    /// Assess, prepare and commit a complete store in one call, as a standalone provider does.
    /// # Errors
    /// Any failure of the three steps; before commitment nothing is charged, reserved or recorded.
    pub fn store(
        &self,
        key: OperationKey,
        caller: IdentityId,
        input: StoreInput,
    ) -> Result<StoreOutput, Error> {
        tracing::trace!("Storing a message");
        let assessment = self.assess_store(caller, input.header())?;
        let prepared = self.prepare_store(key, &assessment, input.message)?;
        match self.commit_store(&prepared) {
            Ok(output) => Ok(output),
            Err(error) => {
                // A failed commit leaves the effect prepared; the caller may retry or abort it. Report the failure as is.
                Err(error)
            }
        }
    }
}

impl Provider {
    /// List the authenticated recipient's pending, unexpired placements: at most `limit` entries ordered by
    /// `(acceptedAt, placementId)` strictly after the optional cursor, with `more` set when another entry follows.
    /// Discovery is read-only and charged the fixed discover price, an empty page included.
    /// # Errors
    /// `Invalid` when the limit is zero or above the offering's maximum. Nothing is charged on failure.
    pub fn discover(
        &self,
        caller: IdentityId,
        input: DiscoverInput,
    ) -> Result<DiscoverOutput, Error> {
        tracing::trace!("Discovering pending placements for the authenticated recipient");
        if input.limit == 0 || input.limit > self.terms.maximum_discover_results {
            return Err(Error::Invalid);
        }
        let now = self.clock.now()?;
        let after = input
            .cursor
            .map(|c| (c.after_accepted_at, c.after_placement_id));
        let wanted = input.limit as usize;
        let mut found = self
            .participant
            .store()
            .pending_for(caller, now, after, wanted + 1);
        let more = found.len() > wanted;
        found.truncate(wanted);
        let messages = found.into_iter().map(entry).collect();
        Ok(DiscoverOutput {
            messages,
            more,
            price: self.terms.discover_price,
        })
    }
}

impl Provider {
    /// Return one pending message to its authenticated recipient. The recovery request is verified (the recipient key derives to the
    /// placement's recipient and its signature covers this provider, placement, message, hash and recovery ID), the exact price is
    /// calculated from the payload size, and the participant atomically records the signed receipt and moves the placement to
    /// recovered. The output carries the original signed message and the receipt, with no price field of its own.
    /// # Errors
    /// `NotAvailable` for an unknown, expired, non-pending or foreign placement and for a message or hash that does not match
    /// (concealed alike so the caller learns nothing); `Invalid` for a bad signature. Nothing is changed or charged on failure.
    pub fn recover(
        &self,
        key: OperationKey,
        caller: IdentityId,
        input: RecoverInput,
    ) -> Result<(RecoverOutput, u64), Error> {
        tracing::trace!("Recovering a pending message for its recipient");
        let effect = match self.plan_recover(caller, input)? {
            RecoverPlan::Replay { output, price } => return Ok((*output, price)),
            RecoverPlan::Fresh(effect) => *effect,
        };
        let token = self.participant.prepare(key, &effect)?;
        let Outcome::Recovered {
            placement_id,
            receipt,
            price,
        } = self.participant.commit(key, &token)?
        else {
            return Err(Error::Conflict);
        };
        let placement = self
            .participant
            .store()
            .get(placement_id)?
            .ok_or(Error::NotAvailable)?;
        Ok((
            RecoverOutput {
                placement_id,
                message: placement.message(),
                receipt,
            },
            price,
        ))
    }

    /// Validate a recovery without changing anything: the request is verified and priced, and either an exact retry is recognized (the
    /// recorded output and original price) or the effect to prepare is returned.
    /// # Errors
    /// As for [`Provider::recover`], before any preparation.
    pub fn plan_recover(
        &self,
        caller: IdentityId,
        input: RecoverInput,
    ) -> Result<RecoverPlan, Error> {
        tracing::trace!("Planning a recovery for its recipient");
        let now = self.clock.now()?;
        let record = self
            .participant
            .store()
            .record(input.placement_id)
            .ok_or(Error::NotAvailable)?;
        let replayed = record
            .receipt
            .is_some_and(|r| r.recovery_id == input.recovery_id);
        let usable = record.active_at(now) && (record.state == State::Pending || replayed);
        let matches = record.metadata.recipient_id == caller
            && record.metadata.message_id == input.message_id
            && record.metadata.payload_hash == input.payload_hash;
        if !usable || !matches {
            return Err(Error::NotAvailable);
        }
        receipt::verify_request(
            &input,
            self.participant.provider_id(),
            record.metadata.recipient_id,
        )?;
        if replayed {
            // An exact retry of the recorded recovery: answer from the record with the original price, charge and effect unchanged.
            let placement = self
                .participant
                .store()
                .get(input.placement_id)?
                .ok_or(Error::NotAvailable)?;
            let receipt = record.receipt.ok_or(Error::NotAvailable)?;
            return Ok(RecoverPlan::Replay {
                output: Box::new(RecoverOutput {
                    placement_id: input.placement_id,
                    message: placement.message(),
                    receipt,
                }),
                price: record.recovery_price,
            });
        }
        let t = &self.terms;
        let price = pricing::recover(
            record.metadata.payload_size,
            t.recovery_usage,
            t.minimum_price,
        )?;
        Ok(RecoverPlan::Fresh(Box::new(Effect::Recover {
            input,
            price,
        })))
    }
}

/// What a validated recovery needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoverPlan {
    /// An exact retry: the recorded output and the original price; nothing is applied or charged again.
    Replay {
        /// The recorded recovery output.
        output: Box<RecoverOutput>,
        /// The original recovery price.
        price: u64,
    },
    /// A first recovery: the effect to prepare and commit.
    Fresh(Box<Effect>),
}

impl Provider {
    /// Tell a placement's sender whether it is pending or recovered, with the exact recorded receipt when recovered and never the
    /// payload. Read-only and charged the fixed status price.
    /// # Errors
    /// `NotAvailable` when the placement is unknown, removed, expired or belongs to another sender (concealed alike).
    pub fn status(&self, caller: IdentityId, input: StatusInput) -> Result<StatusOutput, Error> {
        tracing::trace!("Reporting a placement status to its sender");
        let now = self.clock.now()?;
        let record = self
            .participant
            .store()
            .record(input.placement_id)
            .ok_or(Error::NotAvailable)?;
        if !record.active_at(now) || record.metadata.sender_id != caller {
            return Err(Error::NotAvailable);
        }
        Ok(StatusOutput {
            placement_id: record.id,
            message_id: record.metadata.message_id,
            sender_id: record.metadata.sender_id,
            recipient_id: record.metadata.recipient_id,
            payload_hash: record.metadata.payload_hash,
            state: record.state,
            expires_at: record.expires_at,
            receipt: record.receipt,
            price: self.terms.status_price,
        })
    }
}

impl Provider {
    /// Extend a pending or recovered placement for its sender. The sender must name the expiry it saw; the added duration must be
    /// within the offering's renewal bounds and the new expiry within the total retention allowed from acceptance. The state does
    /// not change and the exact renewal price is charged once.
    /// # Errors
    /// `Invalid` for a duration outside the offering; `NotAvailable` for an unknown, removed, expired or foreign placement;
    /// `Conflict` when the expiry the sender saw is no longer current (a concurrent renewal won); `Limit` when the total retention
    /// or a price would overflow. Nothing changes or is charged on failure.
    pub fn renew(
        &self,
        key: OperationKey,
        caller: IdentityId,
        input: RenewInput,
    ) -> Result<RenewOutput, Error> {
        tracing::trace!("Renewing a placement for its sender");
        let effect = self.plan_renew(caller, input)?;
        let token = self.participant.prepare(key, &effect)?;
        match self.participant.commit(key, &token)? {
            Outcome::Renewed {
                placement_id,
                previous_expires_at,
                expires_at,
                price,
            } => Ok(RenewOutput {
                placement_id,
                previous_expires_at,
                expires_at,
                price,
            }),
            _ => Err(Error::Conflict),
        }
    }

    /// Validate a renewal without changing anything and return the effect to prepare.
    /// # Errors
    /// As for [`Provider::renew`], before any preparation.
    pub fn plan_renew(&self, caller: IdentityId, input: RenewInput) -> Result<Effect, Error> {
        tracing::trace!("Planning a renewal for its sender");
        let t = &self.terms;
        let seconds = input.additional_seconds;
        if seconds < t.minimum_retention_seconds || seconds > t.maximum_retention_seconds {
            return Err(Error::Invalid);
        }
        let now = self.clock.now()?;
        let record = self
            .participant
            .store()
            .record(input.placement_id)
            .ok_or(Error::NotAvailable)?;
        if !record.active_at(now) || record.metadata.sender_id != caller {
            return Err(Error::NotAvailable);
        }
        let ceiling_expires_at = record
            .accepted_at
            .checked_add(t.maximum_total_retention_seconds)
            .ok_or(Error::Limit)?;
        // Checked before preparing so a refusal never leaves a prepared effect holding the placement lock.
        let target = input
            .expected_expires_at
            .checked_add(seconds)
            .ok_or(Error::Limit)?;
        if target > ceiling_expires_at {
            return Err(Error::Limit);
        }
        let price = pricing::renew(
            record.metadata.payload_size,
            seconds,
            t.renewal_storage,
            t.minimum_price,
        )?;
        let effect = Effect::Renew {
            placement_id: input.placement_id,
            expected_expires_at: input.expected_expires_at,
            additional_seconds: seconds,
            ceiling_expires_at,
            price,
        };
        Ok(effect)
    }
}

impl Provider {
    /// Serve one payload byte of a pending placement to its sender as paid evidence that the provider still holds the message.
    /// Read-only; the check price is charged by the Services layer only for a complete valid response.
    /// # Errors
    /// `NotAvailable` for an unknown, inactive (not pending, expired) or foreign placement, concealed alike; `Invalid` for an offset at
    /// or beyond the payload size. Nothing is charged on failure.
    pub fn check(&self, caller: IdentityId, input: CheckInput) -> Result<CheckOutput, Error> {
        tracing::trace!("Serving one payload byte of evidence to the sender");
        let now = self.clock.now()?;
        let record = self
            .participant
            .store()
            .record(input.placement_id)
            .ok_or(Error::NotAvailable)?;
        if record.state != State::Pending
            || !record.active_at(now)
            || record.metadata.sender_id != caller
        {
            return Err(Error::NotAvailable);
        }
        if input.offset >= record.metadata.payload_size {
            return Err(Error::Invalid);
        }
        let placement = self
            .participant
            .store()
            .get(input.placement_id)?
            .ok_or(Error::NotAvailable)?;
        let index = usize::try_from(input.offset).map_err(|_| Error::Invalid)?;
        let value = *placement.payload.get(index).ok_or(Error::Invalid)?;
        Ok(CheckOutput {
            placement_id: input.placement_id,
            offset: input.offset,
            value,
        })
    }
}

/// The terms of one removal, fixed before anything changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemovalQuote {
    /// The exact early-release credits that will be issued to the sender.
    pub release_credits: u64,
    /// The removal charge.
    pub price: u64,
}

impl Provider {
    /// Quote a removal: one exact release amount from the policy, bounded below by the sender's minimum and above by the
    /// placement's cumulative store and renewal payments. Discover, recover, status, remove and check prices are never added to
    /// those payments, so they cannot enlarge the ceiling. Nothing changes.
    /// # Errors
    /// `NotAvailable` unless the placement is pending, unexpired and owned by the sender; `Conflict` when the expiry the sender saw
    /// is stale; `Invalid` when the minimum exceeds the payments; `Limit` when the policy offers less than the minimum.
    pub fn quote_removal(
        &self,
        caller: IdentityId,
        input: &RemoveInput,
    ) -> Result<RemovalQuote, Error> {
        tracing::trace!("Quoting early-release credits for a removal");
        let now = self.clock.now()?;
        let record = self
            .participant
            .store()
            .record(input.placement_id)
            .ok_or(Error::NotAvailable)?;
        if record.state != State::Pending
            || !record.active_at(now)
            || record.metadata.sender_id != caller
        {
            return Err(Error::NotAvailable);
        }
        if record.expires_at != input.expected_expires_at {
            return Err(Error::Conflict);
        }
        let payments = record.payments;
        if input.minimum_release_credits > payments {
            return Err(Error::Invalid);
        }
        let release_credits = self
            .release
            .release(payments, input.minimum_release_credits)
            .min(payments);
        if release_credits < input.minimum_release_credits {
            return Err(Error::Limit);
        }
        Ok(RemovalQuote {
            release_credits,
            price: self.terms.remove_price,
        })
    }
}

impl Provider {
    // The recorded result of this exact removal, if the sender repeats it: the placement is removed with the same removal ID. It is
    // answered from the durable record, with nothing applied again and no expiry check, so a lost reply can be recovered for as
    // long as the record is kept. A different removal ID gets no answer here and falls through to the normal refusal.
    fn recorded_removal(&self, caller: IdentityId, input: &RemoveInput) -> Option<RemoveOutput> {
        let record = self.participant.store().record(input.placement_id)?;
        let removal = record.removal?;
        if record.state != State::Removed
            || record.metadata.sender_id != caller
            || removal.removal_id != input.removal_id
        {
            return None;
        }
        Some(RemoveOutput {
            placement_id: record.id,
            removal_id: removal.removal_id,
            removed: true,
            release_credits: removal.release_credits,
            price: removal.price,
        })
    }

    /// End a pending placement early for its sender: the quote fixes the exact release credits and removal price, then the
    /// participant records the removal and issues the credits exactly once under this operation's key, so recovery can finish an
    /// interrupted settlement without issuing them twice.
    /// # Errors
    /// Those of `quote_removal`, plus `Insufficient`/`Limit` when the credits cannot be issued and `Unsupported` when no issuer is
    /// installed for a positive amount. A refusal before the record changes leaves the placement pending.
    pub fn remove(
        &self,
        key: OperationKey,
        caller: IdentityId,
        input: RemoveInput,
    ) -> Result<RemoveOutput, Error> {
        tracing::trace!("Removing a pending placement for its sender");
        let effect = match self.plan_remove(caller, input)? {
            RemovePlan::Replay(done) => return Ok(done),
            RemovePlan::Fresh(effect) => *effect,
        };
        let token = self.participant.prepare(key, &effect)?;
        match self.participant.commit(key, &token)? {
            Outcome::Removed {
                placement_id,
                removal_id,
                release_credits,
                price,
            } => Ok(RemoveOutput {
                placement_id,
                removal_id,
                removed: true,
                release_credits,
                price,
            }),
            _ => Err(Error::Conflict),
        }
    }

    /// Validate a removal without changing anything: an exact retry is answered from the record, otherwise the quote fixes the credits
    /// and the effect to prepare is returned.
    /// # Errors
    /// Those of `quote_removal`, before any preparation.
    pub fn plan_remove(&self, caller: IdentityId, input: RemoveInput) -> Result<RemovePlan, Error> {
        tracing::trace!("Planning a removal for its sender");
        if let Some(done) = self.recorded_removal(caller, &input) {
            return Ok(RemovePlan::Replay(done));
        }
        let quote = self.quote_removal(caller, &input)?;
        Ok(RemovePlan::Fresh(Box::new(Effect::Remove {
            input,
            release_credits: quote.release_credits,
            price: quote.price,
        })))
    }
}

/// What a validated removal needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemovePlan {
    /// An exact retry: the recorded result; nothing is applied or issued again.
    Replay(RemoveOutput),
    /// A first removal: the effect to prepare and commit.
    Fresh(Box<Effect>),
}

/// The provider-visible description of one placement; the sender is only a hint until recovery verifies it.
fn entry(record: super::placement::Record) -> Entry {
    Entry {
        placement_id: record.id,
        message_id: record.metadata.message_id,
        sender_id: record.metadata.sender_id,
        sender_created_at: record.metadata.sender_created_at,
        encoding: record.metadata.encoding,
        payload_size: record.metadata.payload_size,
        payload_hash: record.metadata.payload_hash,
        accepted_at: record.accepted_at,
        expires_at: record.expires_at,
    }
}

#[cfg(test)]
#[path = "provider_tests.rs"]
pub(super) mod tests;
