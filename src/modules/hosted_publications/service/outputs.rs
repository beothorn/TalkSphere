//! Building Services completions from hosted publication effects and outcomes.
//!
//! A staged *template* is what an effect promises before the coordinator selects a decision time: for a store the acceptance and expiry
//! times are zero. Everything else, including the price and the exact effect bytes, is final. `finalize` fills the decision-time fields;
//! `committed` rebuilds the same result from the outcome the participant recorded.
use crate::modules::{
    hosted_publications::{
        codec::{self, Request, Response},
        effect::{Effect, Outcome},
        error::Error,
        operation::{RemoveOutput, RenewOutput, StoreOutput},
    },
    services::{
        self,
        handler::{Blob, Completion},
        value::{Budget, Value},
    },
};

/// The template response and price of a state-changing effect.
/// # Errors
/// `Invalid` for a query, which has no template; `Limit` for overflow.
pub(crate) fn template(effect: &Effect) -> Result<(Response, u64), Error> {
    tracing::trace!("Building the staged template of an effect");
    match effect {
        Effect::Store {
            placement_id,
            publication,
            price,
            ..
        } => Ok((
            Response::Store(
                StoreOutput {
                    placement_id: *placement_id,
                    publication_id: publication.metadata.publication_id,
                    accepted_at: 0,
                    expires_at: 0,
                },
                *price,
            ),
            *price,
        )),
        Effect::Renew {
            placement_id,
            expected_expires_at,
            additional_seconds,
            price,
            ..
        } => Ok((
            Response::Renew(
                RenewOutput {
                    placement_id: *placement_id,
                    previous_expires_at: *expected_expires_at,
                    expires_at: expected_expires_at
                        .checked_add(*additional_seconds)
                        .ok_or(Error::Limit)?,
                },
                *price,
            ),
            *price,
        )),
        Effect::Remove {
            request,
            release_credits,
            price,
        } => Ok((
            Response::Remove(RemoveOutput {
                placement_id: request.placement_id,
                removal_id: request.removal_id,
                removed: true,
                release_credits: *release_credits,
                price: *price,
            }),
            *price,
        )),
        Effect::Query { .. } => Err(Error::Invalid),
    }
}

/// The decision time carried by a finalized response: only a store has one.
pub(crate) fn decision_time(response: &Response) -> Option<u64> {
    tracing::trace!("Reading the decision time of a response");
    match response {
        Response::Store(output, _) => Some(output.accepted_at),
        _ => None,
    }
}

/// Rebuild the response a recorded outcome produced.
/// # Errors
/// `Conflict` for a query, which has no outcome of its own.
pub(crate) fn committed(outcome: &Outcome) -> Result<Response, Error> {
    tracing::trace!("Rebuilding a response from a recorded outcome");
    Ok(match outcome {
        Outcome::Stored {
            placement_id,
            publication_id,
            accepted_at,
            expires_at,
            price,
        } => Response::Store(
            StoreOutput {
                placement_id: *placement_id,
                publication_id: *publication_id,
                accepted_at: *accepted_at,
                expires_at: *expires_at,
            },
            *price,
        ),
        Outcome::Renewed {
            placement_id,
            previous_expires_at,
            expires_at,
            price,
        } => Response::Renew(
            RenewOutput {
                placement_id: *placement_id,
                previous_expires_at: *previous_expires_at,
                expires_at: *expires_at,
            },
            *price,
        ),
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

/// Wrap a response in a staged completion carrying its exact effect bytes.
/// # Errors
/// Services errors for oversized results.
pub(crate) fn completion(
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

/// Whether an effect is the one a request would produce, so a template cannot be attached to another request.
pub(crate) fn belongs(request: &Request, effect: &Effect) -> bool {
    tracing::trace!("Checking that an effect belongs to its request");
    match (request, effect) {
        (
            Request::Store(r),
            Effect::Store {
                publication,
                retention_seconds,
                ..
            },
        ) => **publication == r.publication && *retention_seconds == r.retention_seconds,
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
        (Request::Remove(r), Effect::Remove { request, .. }) => request == r,
        (Request::List(_) | Request::Get(_), Effect::Query { .. }) => true,
        _ => false,
    }
}

/// Whether a response has exactly the shape an effect promises before finalization.
pub(crate) fn has_template_shape(effect: &Effect, response: &Response, price: u64) -> bool {
    tracing::trace!("Checking the shape of a staged template");
    match (effect, response) {
        (
            Effect::Store {
                placement_id,
                publication,
                price: promised,
                ..
            },
            Response::Store(o, charged),
        ) => {
            *promised == price
                && *charged == price
                && o.placement_id == *placement_id
                && o.publication_id == publication.metadata.publication_id
                && (o.accepted_at, o.expires_at) == (0, 0)
        }
        (
            Effect::Renew {
                placement_id,
                expected_expires_at,
                additional_seconds,
                price: promised,
                ..
            },
            Response::Renew(o, charged),
        ) => {
            *promised == price
                && *charged == price
                && o.placement_id == *placement_id
                && o.previous_expires_at == *expected_expires_at
                && expected_expires_at.checked_add(*additional_seconds) == Some(o.expires_at)
        }
        (
            Effect::Remove {
                request,
                release_credits,
                price: promised,
            },
            Response::Remove(o),
        ) => {
            *promised == price
                && o.placement_id == request.placement_id
                && o.removal_id == request.removal_id
                && o.removed
                && o.release_credits == *release_credits
                && o.price == price
        }
        _ => false,
    }
}
