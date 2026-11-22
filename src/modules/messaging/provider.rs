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
        DiscoverInput, DiscoverOutput, Entry, RecoverInput, RecoverOutput, StatusInput,
        StatusOutput, StoreInput, StoreOutput,
    },
    participant::Participant,
    ports::Clock,
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
        }
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
    /// the offering or the expiry would overflow; `Limit` when the staging budget is full or a price would overflow. Nothing is reserved on failure.
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
        let expected = &assessment.header;
        if message.metadata != expected.metadata
            || message.sender_public_key != expected.sender_public_key
            || message.sender_signature != expected.sender_signature
        {
            return Err(Error::Invalid);
        }
        signing::verify_message(&message, Some(assessment.caller))?;
        let placement_id = self.participant.new_placement_id()?;
        let effect = Effect::Store {
            placement_id,
            message,
            retention_seconds: expected.retention_seconds,
            price: assessment.price,
        };
        let token = self.participant.prepare(key, &effect)?;
        Ok(PreparedStore {
            key,
            token,
            placement_id,
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
            return Ok((
                RecoverOutput {
                    placement_id: input.placement_id,
                    message: placement.message(),
                    receipt,
                },
                record.recovery_price,
            ));
        }
        let t = &self.terms;
        let price = pricing::recover(
            record.metadata.payload_size,
            t.recovery_usage,
            t.minimum_price,
        )?;
        let effect = Effect::Recover { input, price };
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
mod tests;
