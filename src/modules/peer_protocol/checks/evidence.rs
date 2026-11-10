//! Evidence-only schemas and terminal gates never trust a provider-computed correctness outcome.
use super::{clock::Clock, input::Dispatched, *};
use crate::modules::{
    peer_protocol::{
        message::{Kind, Message},
        outbound::Outbound,
        receiver::Receiver as Bodies,
        record::Record,
        sender::Sender as BodySender,
        sequence::Sequence,
        value,
    },
    services::{self, persistence::Persistence, runtime::Coordinator, value::Value},
};
use serde_json::json;
/// Resolve only installed evidence semantics; standard wire shapes are fixed independently from logical check.output.
/// # Errors
/// Missing custom bindings, unknown schema types or provider outcome fields are unsupported/invalid.
pub(super) fn schema(
    service: &str,
    rules: &dyn services::checks::Rules,
) -> Result<serde_json::Value, Error> {
    tracing::trace!("Resolving evidence-only wire shape without correctness claims");
    let schema = match service {
        "cr2se.storage" => {
            json!({"type":"object","fields":{"leaseId":{"type":"bytes","description":"Original challenged 32-byte lease identifier."},"offset":{"type":"uint64","description":"Original challenged byte offset."},"value":{"type":"bytes","description":"Complete one-byte provider evidence."}}})
        }
        "cr2se.messaging" => {
            json!({"type":"object","fields":{"placementId":{"type":"bytes","description":"Original challenged 32-byte placement identifier."},"offset":{"type":"uint64","description":"Original challenged byte offset."},"value":{"type":"bytes","description":"Complete one-byte provider evidence."}}})
        }
        "cr2se.discovery" => {
            json!({"type":"object","fields":{"indexId":{"type":"bytes","description":"Original challenged 32-byte index identifier."},"recordId":{"type":"bytes","description":"Original challenged 32-byte record identifier."},"record":{"type":"bytes","description":"Exact retained signed record bytes."}}})
        }
        _ => rules.evidence_schema()?,
    };
    if services::schema::validate(&schema).map_err(|_| Error::Invalid)?
        != services::schema::Support::Known
        || schema["fields"].get("outcome").is_some()
    {
        return Err(Error::Unsupported);
    }
    Ok(schema)
}
// Exact standard ID/byte widths supplement schema shape; family rules separately validate original challenge/content association.
fn fixed(service: &str, evidence: &Value) -> Result<(), Error> {
    tracing::trace!("Checking exact standard evidence identifier and sample widths");
    let required: &[(&str, usize)] = match service {
        "cr2se.storage" => &[("leaseId", 32), ("value", 1)],
        "cr2se.messaging" => &[("placementId", 32), ("value", 1)],
        "cr2se.discovery" => &[("indexId", 32), ("recordId", 32)],
        _ => return Ok(()),
    };
    let Value::Object(fields) = evidence else {
        return Err(Error::Invalid);
    };
    for (name, length) in required {
        if !fields
            .iter()
            .any(|(n, v)| n == name && matches!(v,Value::Bytes(bytes) if bytes.len()==*length))
        {
            return Err(Error::Invalid);
        }
    }
    Ok(())
}
/// Committed provider evidence sequence; correctness remains a requester-only calculation.
pub struct Sender {
    association: Association,
    evidence: Value,
    sequence: Sequence,
}
impl Sender {
    /// Read only acknowledged original check commitment and its retained wire evidence binding.
    /// # Errors
    /// Missing/pending commitment, changed original context or invalid evidence cannot emit success.
    pub fn new<P: Persistence>(
        context: &Context<'_>,
        authority: &Authority,
        services_context: &services::contract::Context<'_>,
        runtime: &Coordinator<P>,
        dispatched: Dispatched,
        limits: Limits,
    ) -> Result<Self, Error> {
        tracing::trace!("Creating a sender for durably committed provider evidence");
        let a = dispatched.association;
        context.check(authority, a.local, Action::Invoke, 0)?;
        let (invocation, evidence) =
            runtime.completed_check(services_context, dispatched.invocation.key)?;
        if invocation.provider != a.local
            || invocation.requester != a.peer
            || invocation.quote != dispatched.invocation.quote
            || invocation.price != dispatched.invocation.price
        {
            return Err(Error::Denied);
        }
        fixed(&invocation.offering.service, &evidence)?;
        if matches!(&evidence,Value::Object(fields) if fields.iter().any(|(name,_)| name=="outcome"))
        {
            return Err(Error::Invalid);
        }
        let encoded = value::encode(&evidence, limits, false)?;
        let pending = value::Pending::parse(&encoded.metadata, &dispatched.schema, limits, false)?;
        let bodies = BodySender::new(encoded.bodies, pending.descriptors(), limits)?;
        let bytes=serde_json::to_vec(&json!({"type":"result","output":encoded.metadata,"finalPrice":invocation.price.to_string()})).map_err(|_|Error::Invalid)?;
        Message::parse(&bytes, limits.nodes)?;
        context.check(authority, a.local, Action::Invoke, bytes.len())?;
        Ok(Self {
            association: a,
            evidence,
            sequence: Sequence::new(bytes, bodies, b"{\"type\":\"result.end\"}"),
        })
    }
    /// Queue one evidence metadata/body/terminal record under bounded backpressure.
    /// # Errors
    /// Full queues do not advance; failed transmission does not reverse committed evidence accounting.
    pub fn enqueue_next(&mut self, queue: &mut Outbound, chunk: usize) -> Result<bool, Error> {
        tracing::trace!("Queueing committed evidence without a correctness assertion");
        self.sequence.enqueue_next(queue, chunk)
    }
    /// Consume this sender after a trusted local included-response delivery observation, independently of evidence commitment.
    /// # Errors
    /// Complete requires all records drained; paid checks and contradictory/uncertain resets cannot release an included guard.
    pub fn delivery<P: Persistence>(
        self,
        context: &services::contract::Context<'_>,
        runtime: &Coordinator<P>,
        queue: &Outbound,
        delivery: services::checks::Delivery,
    ) -> Result<(), Error> {
        tracing::trace!("Reconciling one included evidence response delivery observation");
        if runtime.local_identity() != self.association.local {
            return Err(Error::Denied);
        }
        if delivery == services::checks::Delivery::Complete
            && (!self.sequence.queued() || queue.pending() != 0)
        {
            return Err(Error::Conflict);
        }
        runtime.check_delivery(
            context,
            super::super::acceptance::peer(self.association).key,
            delivery,
        )?;
        Ok(())
    }
    /// Confirm local marker admission; composition must drain queued bytes before Transport END.
    pub fn queued(&self) -> bool {
        tracing::trace!("Checking committed evidence queue completion");
        self.sequence.queued()
    }
    /// Borrow complete retained evidence without any provider-computed pass/fail outcome.
    pub fn evidence(&self) -> &Value {
        tracing::trace!("Borrowing complete committed provider evidence");
        &self.evidence
    }
    /// Read the original requester stream for action routing.
    pub fn association(&self) -> Association {
        tracing::trace!("Reading original evidence sender association");
        self.association
    }
}
enum Phase {
    Metadata,
    Bodies(Bodies),
    Marked(Bodies),
    Failure(super::super::failure::Failure),
    Failed,
}
/// Requester provisional evidence, bound to exact accepted price and a trusted local clock.
pub struct Receiver {
    authority: Arc<Authority>,
    association: Association,
    invocation: services::checks::Invocation,
    schema: serde_json::Value,
    limits: Limits,
    clock: Arc<dyn Clock>,
    phase: Phase,
    deadline: Instant,
}
impl Receiver {
    /// Bind evidence to one durably dispatched original requester check, not a peer outcome assertion.
    /// # Errors
    /// Invalid limits, wrong authority and cancellation reject receiver creation.
    pub fn new(
        context: &Context<'_>,
        authority: Arc<Authority>,
        outgoing: &Outgoing,
        limits: Limits,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, Error> {
        tracing::trace!("Starting provisional authenticated check evidence reception");
        context.check(&authority, outgoing.association.local, Action::Invoke, 0)?;
        limits.validate()?;
        let invocation = &outgoing.invocation;
        let copy = services::checks::Invocation {
            key: invocation.key,
            requester: invocation.requester,
            provider: invocation.provider,
            issuer: invocation.issuer,
            offering: invocation.offering.clone(),
            definition: invocation.definition.clone(),
            quote: invocation.quote.clone(),
            price: invocation.price,
            response_milliseconds: invocation.response_milliseconds,
            deadline: invocation.deadline,
        };
        Ok(Self {
            authority,
            association: outgoing.association,
            invocation: copy,
            schema: outgoing.schema.clone(),
            limits,
            clock,
            phase: Phase::Metadata,
            deadline: context.deadline,
        })
    }
    /// Accept evidence metadata and exact terminal order without publishing provisional bytes or provider outcomes.
    /// # Errors
    /// Wrong price, provider outcome, malformed schema/body declarations or illegal order poisons this exchange.
    pub fn control(
        &mut self,
        context: &Context<'_>,
        observed: Association,
        message: &Message,
    ) -> Result<(), Error> {
        tracing::trace!("Validating evidence-only metadata and terminal control");
        self.check(context, observed)?;
        context.check(
            &self.authority,
            observed.local,
            Action::Invoke,
            message.bytes().len(),
        )?;
        let previous = std::mem::replace(&mut self.phase, Phase::Failed);
        match (previous, message.kind()) {
            (Phase::Metadata, Kind::Result) => {
                let v = message.value();
                if super::super::scalar::uint64(&v["finalPrice"])? != self.invocation.price
                    || v["output"].get("outcome").is_some()
                {
                    return Err(Error::Invalid);
                }
                let pending =
                    value::Pending::parse(&v["output"], &self.schema, self.limits, false)?;
                self.phase = Phase::Bodies(Bodies::new(pending)?);
                Ok(())
            }
            (Phase::Bodies(receiver), Kind::ResultEnd) if receiver.complete() => {
                self.phase = Phase::Marked(receiver);
                Ok(())
            }
            (Phase::Metadata | Phase::Bodies(_), Kind::Failure) => {
                self.phase = Phase::Failure(super::super::failure::Failure::parse(message)?);
                Ok(())
            }
            _ => Err(Error::Invalid),
        }
    }
    /// Add only contiguous declared evidence body bytes before result.end.
    /// Fragment logs are omitted to avoid per-byte diagnostic amplification.
    /// # Errors
    /// Bad body/order poisons only this receiver; foreign association cannot mutate it.
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
    /// Submit only exact complete evidence after normal original-stream completion with locally observed receive timing.
    /// # Errors
    /// Missing/late/malformed evidence, abnormal close and clock/family/storage failure cannot become check success or charge.
    pub fn finish<P: Persistence>(
        self,
        context: &Context<'_>,
        observed: Association,
        normal: bool,
        services_context: &services::contract::Context<'_>,
        runtime: &Coordinator<P>,
    ) -> Result<Association, Error> {
        tracing::trace!("Submitting complete normally ended evidence with trusted local timing");
        self.check(context, observed)?;
        if !normal {
            return Err(Error::Unknown);
        }
        let peer = super::super::acceptance::peer(observed);
        match self.phase {
            Phase::Marked(receiver) => {
                let evidence = receiver.finish()?;
                fixed(&self.invocation.offering.service, &evidence)?;
                let (_, stamp) = self.clock.mark()?;
                runtime.receive_check(services_context, peer, evidence, stamp)?;
                Ok(observed)
            }
            Phase::Failure(failure) => {
                runtime.decline(services_context, peer)?;
                runtime.abort(services_context, peer.key)?;
                Err(failure.code.error())
            }
            _ => Err(Error::Invalid),
        }
    }
    // Both identity/stream provenance and the captured local call lifetime remain mandatory for each observation.
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
#[cfg(test)]
mod tests {
    use super::*;
    // Standard wire evidence has exact IDs and byte samples, independent of the logical output's requester-only outcome field.
    #[test]
    fn standard_evidence_shapes_and_exact_widths() {
        let rules = crate::modules::peer_protocol::test_support::family::Echo::with_checks(false);
        for (service, id) in [
            ("cr2se.storage", "leaseId"),
            ("cr2se.messaging", "placementId"),
        ] {
            let descriptor = schema(service, &rules).unwrap();
            assert!(descriptor["fields"].get("outcome").is_none());
            let fields = vec![
                (id.into(), Value::Bytes(vec![1; 32])),
                ("offset".into(), Value::Uint64(0)),
                ("value".into(), Value::Bytes(vec![b'x'])),
            ];
            assert_eq!(fixed(service, &Value::Object(fields.clone())), Ok(()));
            for length in [0, 31, 33] {
                let mut bad = fields.clone();
                bad[0].1 = Value::Bytes(vec![1; length]);
                assert_eq!(fixed(service, &Value::Object(bad)), Err(Error::Invalid));
            }
            let mut bad = fields.clone();
            bad[2].1 = Value::Bytes(vec![1; 2]);
            assert_eq!(fixed(service, &Value::Object(bad)), Err(Error::Invalid));
            assert_eq!(fixed(service, &Value::Bytes(vec![1])), Err(Error::Invalid));
        }
        let discovery = Value::Object(vec![
            ("indexId".into(), Value::Bytes(vec![1; 32])),
            ("recordId".into(), Value::Bytes(vec![2; 32])),
            ("record".into(), Value::Bytes(vec![3; 100])),
        ]);
        assert_eq!(fixed("cr2se.discovery", &discovery), Ok(()));
        assert!(
            schema("cr2se.discovery", &rules).unwrap()["fields"]
                .get("outcome")
                .is_none()
        );
        assert_eq!(
            fixed("org.example.peer-test", &Value::Bytes(vec![1])),
            Ok(())
        );
    }
}
