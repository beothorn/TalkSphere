//! Building Services completions from Messaging effects and outcomes.
//!
//! A staged *template* is what an effect promises before the coordinator selects a decision time: for a store the acceptance and
//! expiry times are zero, for a recovery the receipt time and provider signature are placeholders. Everything else, including the
//! price and the exact effect bytes, is final. `finalize` fills the decision-time fields; `committed` rebuilds the same result from
//! the outcome the participant recorded.
use crate::modules::{
    messaging::{
        codec::{self, Response},
        effect::{Effect, Outcome},
        error::Error,
        ids::Signature,
        operation::{Receipt, RecoverOutput, RemoveOutput, RenewOutput, StoreOutput},
        store::Store,
    },
    services::{
        self,
        handler::{Blob, Completion},
        value::{Budget, Value},
    },
};

/// The staged template of a prepared effect and the price it promises.
/// # Errors
/// `Invalid` for a read-only effect (it has no template); `NotAvailable` when a recovery's placement is gone.
pub(super) fn template(effect: &Effect, store: &Store) -> Result<(Response, u64), Error> {
    tracing::trace!("Building the staged template of an effect");
    match effect {
        Effect::Store {
            placement_id,
            message,
            price,
            ..
        } => Ok((
            Response::Store(StoreOutput {
                placement_id: *placement_id,
                message_id: message.metadata.message_id,
                accepted_at: 0,
                expires_at: 0,
                price: *price,
            }),
            *price,
        )),
        Effect::Recover { input, price } => {
            let placement = store.get(input.placement_id)?.ok_or(Error::NotAvailable)?;
            Ok((
                Response::Recover(RecoverOutput {
                    placement_id: input.placement_id,
                    message: placement.message(),
                    receipt: Receipt {
                        recovery_id: input.recovery_id,
                        recovered_at: 0,
                        recipient_public_key: input.recipient_public_key,
                        recipient_signature: input.recipient_signature,
                        provider_signature: Signature([0; 64]),
                    },
                }),
                *price,
            ))
        }
        Effect::Renew {
            placement_id,
            expected_expires_at,
            additional_seconds,
            price,
            ..
        } => Ok((
            Response::Renew(RenewOutput {
                placement_id: *placement_id,
                previous_expires_at: *expected_expires_at,
                expires_at: expected_expires_at
                    .checked_add(*additional_seconds)
                    .ok_or(Error::Limit)?,
                price: *price,
            }),
            *price,
        )),
        Effect::Remove {
            input,
            release_credits,
            price,
        } => Ok((
            Response::Remove(RemoveOutput {
                placement_id: input.placement_id,
                removal_id: input.removal_id,
                removed: true,
                release_credits: *release_credits,
                price: *price,
            }),
            *price,
        )),
        Effect::Query { .. } => Err(Error::Invalid),
    }
}

/// The decision time carried by a finalized response: acceptance for a store, the receipt time for a recovery.
pub(super) fn decision_time(response: &Response) -> Option<u64> {
    tracing::trace!("Reading the decision time of a response");
    match response {
        Response::Store(output) => Some(output.accepted_at),
        Response::Recover(output) => Some(output.receipt.recovered_at),
        _ => None,
    }
}

/// The response the participant's recorded outcome stands for.
/// # Errors
/// `Conflict` when the outcome does not belong to the operation; `NotAvailable` when a recovered message is gone.
pub(super) fn committed(outcome: &Outcome, store: &Store) -> Result<Response, Error> {
    tracing::trace!("Rebuilding a response from a recorded outcome");
    Ok(match outcome {
        Outcome::Stored {
            placement_id,
            message_id,
            accepted_at,
            expires_at,
            price,
        } => Response::Store(StoreOutput {
            placement_id: *placement_id,
            message_id: *message_id,
            accepted_at: *accepted_at,
            expires_at: *expires_at,
            price: *price,
        }),
        Outcome::Recovered {
            placement_id,
            receipt,
            ..
        } => {
            let placement = store.get(*placement_id)?.ok_or(Error::NotAvailable)?;
            Response::Recover(RecoverOutput {
                placement_id: *placement_id,
                message: placement.message(),
                receipt: *receipt,
            })
        }
        Outcome::Renewed {
            placement_id,
            previous_expires_at,
            expires_at,
            price,
        } => Response::Renew(RenewOutput {
            placement_id: *placement_id,
            previous_expires_at: *previous_expires_at,
            expires_at: *expires_at,
            price: *price,
        }),
        Outcome::Removed {
            placement_id,
            removal_id,
            release_credits,
            price,
        } => Response::Remove(RemoveOutput {
            placement_id: *placement_id,
            removal_id: *removal_id,
            removed: true,
            release_credits: *release_credits,
            price: *price,
        }),
        Outcome::Query => return Err(Error::Conflict),
    })
}

/// Wrap a response and its price in a Services completion carrying the effect bytes.
/// # Errors
/// `Limit` when the encoded result or effect exceeds `maximum_bytes`.
pub(super) fn completion(
    response: &Response,
    price: u64,
    effect: &Effect,
    budget: Budget,
    maximum_bytes: usize,
) -> Result<Completion, services::Error> {
    tracing::trace!("Wrapping a response in a staged completion");
    let output: Value = codec::response_value(response);
    let result = services::codec::encode(&output, budget, maximum_bytes)?;
    Ok(Completion {
        result: Blob::new(result, maximum_bytes)?,
        output,
        price,
        effect: Blob::new(effect.encode(), maximum_bytes)?,
    })
}
