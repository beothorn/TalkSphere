//! Building Services completions from Messaging effects and outcomes.
//!
//! A staged *template* is what an effect promises before the coordinator selects a decision time: for a store the acceptance and
//! expiry times are zero, for a recovery the receipt time and provider signature are placeholders. Everything else, including the
//! price and the exact effect bytes, is final. `finalize` fills the decision-time fields; `committed` rebuilds the same result from
//! the outcome the participant recorded.
use crate::modules::{
    messaging::{
        codec::{self, Request, Response},
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

/// Whether an effect is the effect of exactly this request.
pub(super) fn belongs(request: &Request, effect: &Effect) -> bool {
    tracing::trace!("Checking that an effect belongs to its request");
    match (request, effect) {
        (
            Request::Store(r),
            Effect::Store {
                message,
                retention_seconds,
                ..
            },
        ) => *message == r.message && *retention_seconds == r.retention_seconds,
        (Request::Recover(r), Effect::Recover { input, .. }) => input == r,
        (
            Request::Renew(r),
            Effect::Renew {
                placement_id,
                expected_expires_at,
                additional_seconds,
                ..
            },
        ) => {
            *placement_id == r.placement_id
                && *expected_expires_at == r.expected_expires_at
                && *additional_seconds == r.additional_seconds
        }
        (Request::Remove(r), Effect::Remove { input, .. }) => input == r,
        _ => false,
    }
}

/// Whether `response` is exactly the staged template an effect promises at `price`, without reading any live state.
pub(super) fn has_template_shape(effect: &Effect, response: &Response, price: u64) -> bool {
    tracing::trace!("Checking the shape of a staged template");
    match (effect, response) {
        (
            Effect::Store {
                placement_id,
                message,
                price: promised,
                ..
            },
            Response::Store(o),
        ) => {
            *promised == price
                && o.placement_id == *placement_id
                && o.message_id == message.metadata.message_id
                && (o.accepted_at, o.expires_at, o.price) == (0, 0, price)
        }
        (
            Effect::Recover {
                input,
                price: promised,
            },
            Response::Recover(o),
        ) => {
            *promised == price
                && o.placement_id == input.placement_id
                && o.message.metadata.message_id == input.message_id
                && o.message.metadata.payload_hash == input.payload_hash
                && o.receipt.recovery_id == input.recovery_id
                && o.receipt.recovered_at == 0
                && o.receipt.provider_signature.0 == [0; 64]
                && o.receipt.recipient_public_key == input.recipient_public_key
                && o.receipt.recipient_signature == input.recipient_signature
        }
        (
            Effect::Renew {
                placement_id,
                expected_expires_at,
                additional_seconds,
                price: promised,
                ..
            },
            Response::Renew(o),
        ) => {
            *promised == price
                && o.placement_id == *placement_id
                && o.previous_expires_at == *expected_expires_at
                && expected_expires_at.checked_add(*additional_seconds) == Some(o.expires_at)
                && o.price == price
        }
        (
            Effect::Remove {
                input,
                release_credits,
                price: promised,
            },
            Response::Remove(o),
        ) => {
            *promised == price
                && o.placement_id == input.placement_id
                && o.removal_id == input.removal_id
                && o.removed
                && o.release_credits == *release_credits
                && o.price == price
        }
        _ => false,
    }
}
