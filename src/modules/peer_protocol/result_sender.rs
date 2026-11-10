//! Result transmission reads only committed Services output and retains immutable sources across stream failure.
use super::{
    Error,
    authority::{Action, Authority},
    contract::{Association, Context, Limits},
    outbound::Outbound,
    sender::Sender,
    sequence::Sequence,
    value,
};
use crate::modules::services::{
    self, handler::Completion, persistence::Persistence, runtime::Coordinator,
};
use serde_json::json;
use std::sync::Arc;
/// Ordered committed result metadata, bodies and terminal marker, independent of a caller-session lifetime.
pub struct ResultSender {
    association: Association,
    completion: Completion,
    sequence: Sequence,
}
impl ResultSender {
    /// Read original-provider output only after all local participant commitments are acknowledged.
    /// # Errors
    /// Missing/uncertain commitment, foreign association, bad output price or bounds fails before emitting any result byte.
    pub fn new<P: Persistence>(
        context: &Context<'_>,
        authority: &Arc<Authority>,
        services_context: &services::contract::Context<'_>,
        runtime: &Coordinator<P>,
        association: Association,
        limits: Limits,
    ) -> Result<Self, Error> {
        tracing::trace!("Creating a result sender from durably committed Services output");
        context.check(authority, association.local, Action::Invoke, 0)?;
        let (invocation, completion) =
            runtime.completed(services_context, super::acceptance::peer(association).key)?;
        if invocation.provider != association.local || invocation.requester != association.peer {
            return Err(Error::Denied);
        }
        services::pricing::validate_output_price(&completion.output, completion.price)?;
        let encoded = value::encode(&completion.output, limits, true)?;
        let pending = value::Pending::parse(
            &encoded.metadata,
            &invocation.definition.document.value()["output"],
            limits,
            true,
        )?;
        let bodies = Sender::new(encoded.bodies, pending.descriptors(), limits)?;
        let wire=serde_json::to_vec(&json!({"type":"result","output":encoded.metadata,"finalPrice":completion.price.to_string()})).map_err(|_|Error::Invalid)?;
        super::message::Message::parse(&wire, limits.nodes)?;
        context.check(authority, association.local, Action::Invoke, wire.len())?;
        Ok(Self {
            association,
            completion,
            sequence: Sequence::new(wire, bodies, b"{\"type\":\"result.end\"}"),
        })
    }
    /// Queue one metadata/body/marker record without altering durable result or payment ownership.
    /// # Errors
    /// Backpressure leaves progress unchanged; local transmission failure cannot turn committed work into an uncharged failure.
    pub fn enqueue_next(&mut self, queue: &mut Outbound, chunk: usize) -> Result<bool, Error> {
        tracing::trace!("Queueing the next committed result record");
        self.sequence.enqueue_next(queue, chunk)
    }
    /// Confirm every record is locally queued; send Transport END only after that queue drains.
    pub fn queued(&self) -> bool {
        tracing::trace!("Checking whether committed result terminal marker is queued");
        self.sequence.queued()
    }
    /// Read original endpoint/stream association for action routing.
    pub fn association(&self) -> Association {
        tracing::trace!("Reading committed result stream association");
        self.association
    }
    /// Borrow retained complete committed output; dropping a sender never removes coordinator storage.
    pub fn completion(&self) -> &Completion {
        tracing::trace!("Borrowing retained committed output");
        &self.completion
    }
}
