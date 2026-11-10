//! Requester output remains provisional until exact bodies, result.end and normal original-stream completion.
use super::{
    Error,
    acceptance::Outgoing,
    authority::{Action, Authority},
    contract::{Association, Context, Limits},
    message::{Kind, Message},
    receiver::Receiver,
    record::Record,
    value::Pending,
};
use crate::modules::services::{
    self,
    handler::{Blob, Invocation},
    persistence::Persistence,
    runtime::{Coordinator, RemoteResult},
    value::Budget,
};
use std::{sync::Arc, time::Instant};
enum Phase {
    Metadata,
    Bodies(Receiver),
    Marked(Receiver),
    Failed,
}
/// No successful output accessor exists; completed data must pass the original coordinator's independent validation.
pub struct ResultReceiver {
    authority: Arc<Authority>,
    association: Association,
    invocation: Invocation,
    deadline: Instant,
    limits: Limits,
    phase: Phase,
    price: Option<u64>,
}
impl ResultReceiver {
    /// Bind one original reserved requester attempt before receiving provider output.
    /// # Errors
    /// Wrong purpose/roles, invalid limits and cancellation fail before any body allocation.
    pub fn new(
        context: &Context<'_>,
        authority: Arc<Authority>,
        outgoing: &Outgoing,
        limits: Limits,
    ) -> Result<Self, Error> {
        tracing::trace!("Starting provisional original-provider result reception");
        let a = outgoing.association();
        context.check(&authority, a.local, Action::Invoke, 0)?;
        limits.validate()?;
        if outgoing.invocation().requester != a.local || outgoing.invocation().provider != a.peer {
            return Err(Error::Denied);
        }
        Ok(Self {
            authority,
            association: a,
            invocation: outgoing.invocation().clone(),
            deadline: context.deadline,
            limits,
            phase: Phase::Metadata,
            price: None,
        })
    }
    /// Receive result metadata once, then require complete bodies before result.end; no result escapes provisionally.
    /// # Errors
    /// Wrong association cannot mutate this stream; malformed/order/price failures discard its provisional value.
    pub fn control(
        &mut self,
        context: &Context<'_>,
        observed: Association,
        message: &Message,
    ) -> Result<(), Error> {
        tracing::trace!("Validating provisional result metadata or terminal marker");
        self.check(context, observed)?;
        context.check(
            &self.authority,
            self.association.local,
            Action::Invoke,
            message.bytes().len(),
        )?;
        let previous = std::mem::replace(&mut self.phase, Phase::Failed);
        match (previous, message.kind()) {
            (Phase::Metadata, Kind::Result) => {
                let price = super::scalar::uint64(&message.value()["finalPrice"])?;
                let maximum = self.invocation.pricing.maximum(
                    &self.invocation.offering.service,
                    self.invocation.offering.version,
                )?;
                if price > maximum
                    || (matches!(self.invocation.pricing, services::pricing::Price::Fixed(_))
                        && price != maximum)
                {
                    return Err(Error::Invalid);
                }
                let pending = Pending::parse(
                    &message.value()["output"],
                    &self.invocation.definition.document.value()["output"],
                    self.limits,
                    true,
                )?;
                self.phase = Phase::Bodies(Receiver::new(pending)?);
                self.price = Some(price);
                Ok(())
            }
            (Phase::Bodies(receiver), Kind::ResultEnd) if receiver.complete() => {
                self.phase = Phase::Marked(receiver);
                Ok(())
            }
            _ => Err(Error::Invalid),
        }
    }
    /// Receive only declared ordered output bytes between result and result.end.
    /// Fragment logging is omitted to avoid attacker-controlled per-byte diagnostic volume.
    /// # Errors
    /// Early/late/extra data or malformed body bytes poison only this exchange.
    pub fn body(
        &mut self,
        context: &Context<'_>,
        observed: Association,
        record: &Record,
    ) -> Result<(), Error> {
        self.check(context, observed)?;
        let result = match &mut self.phase {
            Phase::Bodies(receiver) => receiver.push(record),
            _ => Err(Error::Invalid),
        };
        if result.is_err() {
            self.phase = Phase::Failed;
        }
        result
    }
    /// Submit complete output to Services only after normal Transport completion of the original exchange.
    /// # Errors
    /// Abnormal completion remains Unknown; missing bodies/markers, mismatched price, content/family failure and storage uncertainty cannot publish success.
    pub fn finish<P: Persistence>(
        self,
        context: &Context<'_>,
        observed: Association,
        normal: bool,
        services_context: &services::contract::Context<'_>,
        runtime: &Coordinator<P>,
    ) -> Result<Association, Error> {
        tracing::trace!("Submitting original provider result only after normal stream completion");
        self.check(context, observed)?;
        if !normal {
            return Err(Error::Unknown);
        }
        let Phase::Marked(receiver) = self.phase else {
            return Err(Error::Invalid);
        };
        let output = receiver.finish()?;
        let price = self.price.ok_or(Error::Invalid)?;
        services::pricing::validate_output_price(&output, price)?;
        // This is local durable logical-value storage, not a replacement for the Peer Protocol wire codec.
        let bytes = services::codec::encode(
            &output,
            Budget {
                depth: 31,
                nodes: self.limits.nodes,
                bytes: self.limits.state_bytes,
            },
            self.limits.state_bytes,
        )?;
        runtime.receive(
            services_context,
            super::acceptance::peer(self.association),
            RemoteResult {
                output,
                bytes: Blob::new(bytes, self.limits.state_bytes)?,
                price,
            },
        )?;
        Ok(self.association)
    }
    // Trusted composition must route only the authenticated original provider and captured local deadline to this gate.
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
