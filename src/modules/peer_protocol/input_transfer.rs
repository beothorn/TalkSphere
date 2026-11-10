//! Accepted input remains provisional until exact bodies, input.end and requester END; only then may Services dispatch.
use super::{
    Error,
    acceptance::{Incoming, Outgoing},
    authority::{Action, Authority},
    contract::{Association, Context, Limits},
    message::{Kind, Message},
    outbound::Outbound,
    receiver::Receiver,
    record::Record,
    sender::Sender,
    value,
};
use crate::modules::services::{
    self, persistence::Persistence, runtime::Coordinator, value::Value,
};
use std::{sync::Arc, time::Instant};

/// Provider-side accepted input gate with separate metadata marker and Transport half-close requirements.
pub struct InputTransfer {
    authority: Arc<Authority>,
    association: Association,
    deadline: Instant,
    receiver: Option<Receiver>,
    marked: bool,
}
impl InputTransfer {
    /// Start bounded provisional reception only from an opaque successfully reserved provider attempt.
    /// # Errors
    /// Missing invocation authority, cancellation and invalid receiver capacity fail before dispatch.
    pub fn new(
        context: &Context<'_>,
        authority: Arc<Authority>,
        accepted: Incoming,
    ) -> Result<Self, Error> {
        tracing::trace!("Starting reserved provider input reception");
        context.check(&authority, accepted.association.local, Action::Invoke, 0)?;
        Ok(Self {
            authority,
            association: accepted.association,
            deadline: context.deadline,
            receiver: Some(Receiver::new(accepted.input)?),
            marked: false,
        })
    }
    /// Add the next body before input.end; no complete value or execution becomes visible.
    /// Fragment entry logs are omitted to bound diagnostic volume under one-byte delivery.
    /// # Errors
    /// Wrong association/authority fails without mutation; invalid body/order poisons only this transfer.
    pub fn body(
        &mut self,
        context: &Context<'_>,
        observed: Association,
        record: &Record,
    ) -> Result<(), Error> {
        self.check(context, observed)?;
        if self.marked {
            self.receiver = None;
            return Err(Error::Invalid);
        }
        self.receiver.as_mut().ok_or(Error::Conflict)?.push(record)
    }
    /// Require all exact bodies before accepting one input.end control, without treating it as Transport END.
    /// # Errors
    /// Early, duplicate or wrong controls discard provisional buffers and forbid dispatch.
    pub fn marker(
        &mut self,
        context: &Context<'_>,
        observed: Association,
        message: &Message,
    ) -> Result<(), Error> {
        tracing::trace!("Validating complete input before the input.end marker");
        self.check(context, observed)?;
        if self.marked
            || message.kind() != Kind::InputEnd
            || !self.receiver.as_ref().is_some_and(Receiver::complete)
        {
            self.receiver = None;
            return Err(Error::Invalid);
        }
        self.marked = true;
        Ok(())
    }
    /// Consume the gate on the original requester's Transport END and publish one complete input token.
    /// # Errors
    /// Missing marker/bodies, malformed UTF-8, cancellation or wrong provenance cannot produce a dispatch token.
    pub fn end(self, context: &Context<'_>, observed: Association) -> Result<CompleteInput, Error> {
        tracing::trace!("Completing reserved input after requester Transport END");
        self.check(context, observed)?;
        if !self.marked {
            return Err(Error::Invalid);
        }
        let input = self.receiver.ok_or(Error::Conflict)?.finish()?;
        Ok(CompleteInput {
            association: self.association,
            input,
        })
    }
    // The original authenticated stream and finite local deadline remain bound across every callback.
    fn check(&self, context: &Context<'_>, observed: Association) -> Result<(), Error> {
        context.check(&self.authority, self.association.local, Action::Invoke, 0)?;
        if observed != self.association {
            return Err(Error::Denied);
        }
        if Instant::now() >= self.deadline {
            return Err(Error::Cancelled);
        }
        Ok(())
    }
}
/// Complete original input after all protocol terminal requirements, still subject to independent family validation.
pub struct CompleteInput {
    association: Association,
    input: Value,
}
impl CompleteInput {
    /// Borrow only fully assembled text/bytes and schema-bound values, never a provisional body.
    pub fn value(&self) -> &Value {
        tracing::trace!("Borrowing input completed through both terminal requirements");
        &self.input
    }
    /// Validate complete input through Services and persist the once-only execution marker before invoking the provider.
    /// # Errors
    /// Content/hash/family refusal, wrong grants and storage/dispatch failures preserve durable recovery obligations.
    pub fn dispatch<P: Persistence>(
        self,
        context: &services::contract::Context<'_>,
        runtime: &Coordinator<P>,
    ) -> Result<Association, Error> {
        tracing::trace!("Dispatching only complete terminally accepted service input");
        let key = super::acceptance::peer(self.association).key;
        runtime.supply_input(context, key, self.input)?;
        runtime.start(context, key)?;
        Ok(self.association)
    }
}
/// Requester outbound order includes accept before any detached bytes and input.end only after every body is queued.
pub struct InputSender {
    outgoing: Outgoing,
    bodies: Sender,
    stage: u8,
}
impl InputSender {
    /// Retain reserved original input and immutable sources; failure cannot silently release its journal/hold.
    /// # Errors
    /// Encoding/declaration/resource failures reject local transmission without claiming rollback.
    pub fn new(outgoing: Outgoing, limits: Limits) -> Result<Self, Error> {
        tracing::trace!("Creating ordered accepted requester input sender");
        let encoded = value::encode(&outgoing.input, limits, true)?;
        let pending = value::Pending::parse(
            &encoded.metadata,
            &outgoing.invocation.definition.document.value()["input"],
            limits,
            true,
        )?;
        let bodies = Sender::new(encoded.bodies, pending.descriptors(), limits)?;
        Ok(Self {
            outgoing,
            bodies,
            stage: 0,
        })
    }
    /// Enqueue one accept/body/marker in protocol order under the caller's bounded queue.
    /// # Errors
    /// Backpressure leaves progress unchanged; invalid chunk/queue errors never imply remote receipt.
    pub fn enqueue_next(&mut self, queue: &mut Outbound, chunk: usize) -> Result<bool, Error> {
        tracing::trace!("Queueing the next ordered accepted input record");
        if self.stage == 0 {
            queue.enqueue(&Record::Control(self.outgoing.accept().to_vec()))?;
            self.stage = 1;
            return Ok(true);
        }
        if self.stage == 1 {
            if self.bodies.enqueue_next(queue, chunk)? {
                return Ok(true);
            }
            queue.enqueue(&Record::Control(b"{\"type\":\"input.end\"}".to_vec()))?;
            self.stage = 2;
            return Ok(true);
        }
        Ok(false)
    }
    /// Confirm that all records are locally queued; send END only after the queue has fully drained.
    pub fn queued(&self) -> bool {
        tracing::trace!("Checking whether input terminal marker is queued");
        self.stage == 2
    }
    /// Borrow original accepted invocation for later authenticated result handling independent of sender buffering.
    pub fn outgoing(&self) -> &Outgoing {
        tracing::trace!("Borrowing accepted original requester attempt");
        &self.outgoing
    }
}
