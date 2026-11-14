//! Check challenges contain no private expectations; response timing starts at input.end rather than proposal or quote.
use super::{clock::Clock, *};
use crate::modules::peer_protocol::{
    message::{Kind, Message},
    outbound::Outbound,
    sequence::Sequence,
};
use crate::modules::services::{persistence::Persistence, runtime::Coordinator};
use std::time::Duration;
/// Provider challenge terminal gate; the challenge was fully bounded and eligibility-checked before quoting.
pub struct Receiver {
    authority: Arc<Authority>,
    incoming: Incoming,
    clock: Arc<dyn Clock>,
    marked: Option<Instant>,
    failed: bool,
}
/// Once-dispatched original provider check context retained for committed evidence transmission.
pub struct Dispatched {
    pub(super) association: Association,
    pub(super) invocation: services::checks::Invocation,
    pub(super) schema: serde_json::Value,
}
impl Receiver {
    /// Start reception only after an actual provider check reservation.
    /// # Errors
    /// Wrong authority/cancellation cannot create a terminal gate.
    pub fn new(
        context: &Context<'_>,
        authority: Arc<Authority>,
        incoming: Incoming,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, Error> {
        tracing::trace!("Starting accepted challenge terminal gating");
        context.check(&authority, incoming.association.local, Action::Invoke, 0)?;
        Ok(Self {
            authority,
            incoming,
            clock,
            marked: None,
            failed: false,
        })
    }
    /// Observe exactly input.end and capture the response deadline before waiting for requester END.
    /// # Errors
    /// Wrong stream, duplicate/wrong marker and clock/overflow failures cannot dispatch evidence.
    pub fn marker(
        &mut self,
        context: &Context<'_>,
        observed: Association,
        message: &Message,
    ) -> Result<(), Error> {
        tracing::trace!("Capturing a trusted direct-call check marker instant");
        self.validate_marker(context, observed, message)?;
        let (received_at, _) = self.clock.mark()?;
        self.set_marker(received_at)
    }
    /// Start response timing at the original authenticated record receipt, preserving asynchronous queue delay.
    /// `received_at` must come from trusted Transport composition, never peer JSON or a worker's later clock.
    /// # Errors
    /// Foreign authority, wrong/duplicate markers and deadline overflow fail without dispatch.
    pub fn marker_at(
        &mut self,
        context: &Context<'_>,
        observed: Association,
        message: &Message,
        received_at: Instant,
    ) -> Result<(), Error> {
        tracing::trace!("Starting check timing at the authenticated record receipt instant");
        self.validate_marker(context, observed, message)?;
        self.set_marker(received_at)
    }
    // Validate authority and marker order before consulting a clock or changing response timing.
    fn validate_marker(
        &mut self,
        context: &Context<'_>,
        observed: Association,
        message: &Message,
    ) -> Result<(), Error> {
        tracing::trace!("Validating an original check marker before timing effects");
        self.check(context, observed)?;
        if self.failed || self.marked.is_some() || message.kind() != Kind::InputEnd {
            self.failed = true;
            return Err(Error::Invalid);
        }
        Ok(())
    }
    // Preserve the original receive instant even when execution is delayed on a worker queue.
    fn set_marker(&mut self, received_at: Instant) -> Result<(), Error> {
        tracing::trace!("Retaining an original check response deadline");
        self.marked = Some(
            received_at
                .checked_add(Duration::from_millis(
                    self.incoming.invocation.response_milliseconds,
                ))
                .ok_or(Error::Limit)?,
        );
        Ok(())
    }
    /// Dispatch only after original requester END with the deadline already captured at input.end.
    /// # Errors
    /// Missing marker, expired timing, family challenge refusal and durable/execution uncertainty cannot become evidence success.
    pub fn end<P: Persistence>(
        self,
        context: &Context<'_>,
        observed: Association,
        services_context: &services::contract::Context<'_>,
        runtime: &Coordinator<P>,
    ) -> Result<Dispatched, Error> {
        tracing::trace!("Dispatching accepted check after both input terminal signals");
        self.check(context, observed)?;
        if self.failed {
            return Err(Error::Invalid);
        }
        let deadline = self
            .marked
            .ok_or(Error::Invalid)?
            .min(context.deadline)
            .min(services_context.deadline)
            .min(self.incoming.invocation.deadline);
        let bounded = services::contract::Context {
            version: services_context.version,
            grant: services_context.grant,
            deadline,
            cancelled: services_context.cancelled,
        };
        let key = self.incoming.invocation.key;
        runtime.supply_input(&bounded, key, self.incoming.challenge)?;
        runtime.start(&bounded, key)?;
        Ok(Dispatched {
            association: observed,
            invocation: self.incoming.invocation,
            schema: self.incoming.schema,
        })
    }
    // Event provenance is checked before mutating this accepted exchange's local timing.
    fn check(&self, context: &Context<'_>, observed: Association) -> Result<(), Error> {
        context.check(
            &self.authority,
            self.incoming.association.local,
            Action::Invoke,
            0,
        )?;
        if observed != self.incoming.association {
            return Err(Error::Denied);
        }
        Ok(())
    }
}
/// Requester accept/input.end sequence; no body or expected byte is transmitted.
pub struct Sender {
    outgoing: Outgoing,
    sequence: Sequence,
    sent: bool,
}
impl Sender {
    /// Retain the already reserved original challenge and sequence its accept before input.end.
    /// # Errors
    /// Invalid finite limits reject local sender creation without releasing accepted obligations.
    pub fn new(outgoing: Outgoing, limits: Limits) -> Result<Self, Error> {
        tracing::trace!("Creating a challenge-only accepted input sender");
        let bodies = super::super::sender::Sender::new(Vec::new(), &[], limits)?;
        let sequence = Sequence::new(outgoing.accept.clone(), bodies, b"{\"type\":\"input.end\"}");
        Ok(Self {
            outgoing,
            sequence,
            sent: false,
        })
    }
    /// Queue one accept/input.end record without introducing private expectations or binary challenge data.
    /// # Errors
    /// Bounded backpressure leaves progress unchanged.
    pub fn enqueue_next(&mut self, queue: &mut Outbound) -> Result<bool, Error> {
        tracing::trace!("Queueing accepted check input controls");
        self.sequence.enqueue_next(queue, 1)
    }
    /// Record the complete input.end send observation only after all local records have drained.
    /// # Errors
    /// Premature/duplicate send observations and clock/coordinator failures cannot fabricate response timing.
    pub fn mark_sent<P: Persistence>(
        &mut self,
        queue: &Outbound,
        context: &services::contract::Context<'_>,
        runtime: &Coordinator<P>,
        clock: &dyn Clock,
    ) -> Result<(), Error> {
        tracing::trace!("Recording complete challenge send timing after input.end drains");
        if !self.sequence.queued() || queue.pending() != 0 || self.sent {
            return Err(Error::Conflict);
        }
        let (_, stamp) = clock.mark()?;
        self.mark_sent_at(queue, context, runtime, stamp)
    }
    /// Record the original local output-drain stamp after all check input records were admitted by Transport.
    /// The stamp comes from trusted composition in the same stable local clock epoch, never a peer timestamp.
    /// # Errors
    /// Premature/duplicate observation, wrong authority or owner persistence failure cannot record a fresh send.
    pub fn mark_sent_at<P: Persistence>(
        &mut self,
        queue: &Outbound,
        context: &services::contract::Context<'_>,
        runtime: &Coordinator<P>,
        stamp: services::checks::Stamp,
    ) -> Result<(), Error> {
        tracing::trace!("Retaining original complete challenge send timing across worker delay");
        if !self.sequence.queued() || queue.pending() != 0 || self.sent {
            return Err(Error::Conflict);
        }
        runtime.check_sent(context, self.outgoing.invocation.key, stamp)?;
        self.sent = true;
        Ok(())
    }
    /// Borrow original requester acceptance for evidence binding; this contains challenge only.
    pub fn outgoing(&self) -> &Outgoing {
        tracing::trace!("Borrowing original accepted requester check");
        &self.outgoing
    }
}
