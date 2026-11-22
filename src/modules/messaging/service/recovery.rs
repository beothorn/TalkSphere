//! Startup recovery of Messaging work through the Services coordinator.
//!
//! Callers: node composition, after the participant has recovered deferring to Services, the handlers are registered and the
//! coordinator is open, and before the offering is published. Every unfinished Messaging operation is resumed from the coordinator's
//! own durable record: a recorded commit is delivered together with its retained decision time, a recorded abort is released, and an
//! operation the participant never saw is aborted. Nothing is executed again and nothing is decided here.
use crate::modules::services::{
    Error,
    contract::{Context, OperationKey},
    handler::{Provider, Status},
    persistence::Persistence,
    runtime::{Coordinator, Phase},
};

/// The Messaging service name and version the coordinator tracks.
const SERVICE: &str = "cr2se.messaging";
const VERSION: u32 = 1;

/// Resume every unfinished Messaging operation and report how many reached a final decision.
/// # Errors
/// `Unknown` when an operation does not reach a final decision (it stays recoverable and recovery can be run again); the coordinator's
/// and the participant's errors otherwise.
pub fn resume<P: Persistence>(
    coordinator: &Coordinator<P>,
    context: &Context<'_>,
    provider: &dyn Provider,
) -> Result<usize, Error> {
    tracing::trace!("Resuming unfinished Messaging operations through Services");
    let mut finished = 0;
    for key in coordinator.pending_provider(context, SERVICE, VERSION)? {
        if resume_one(coordinator, context, provider, key)? {
            finished += 1;
        }
    }
    Ok(finished)
}

// Resume one operation: an absent participant means nothing was ever prepared, otherwise the coordinator re-delivers its decision.
fn resume_one<P: Persistence>(
    coordinator: &Coordinator<P>,
    context: &Context<'_>,
    provider: &dyn Provider,
    key: OperationKey,
) -> Result<bool, Error> {
    let phase = match provider.status(key)? {
        Status::Absent => coordinator.recover_absent_provider(context, key, SERVICE, VERSION)?,
        _ => coordinator.resume(context, key)?,
    };
    Ok(matches!(phase, Phase::Committed | Phase::Aborted))
}
