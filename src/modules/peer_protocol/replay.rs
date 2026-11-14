//! Explicit Messaging recovery/removal retransmission bypasses new prices only after original family/caller/request validation.
use super::{
    Error,
    authority::{Action, Authority},
    contract::{Association, Context, Limits},
    message::{Kind, Message},
    outbound::Outbound,
    scalar,
    sender::Sender as Bodies,
    sequence::Sequence,
    value::{self, Pending},
};
use crate::modules::{
    identity::text,
    services::{
        self,
        persistence::Persistence,
        registry::Registry,
        replay::{Cached, Proposal, Retry},
        runtime::Coordinator,
        value::Budget,
    },
};
use serde_json::json;
/// Separately scoped recovery/observation ports; these do not authorize new work, registration or spending.
pub struct Lookup<'a, P: Persistence> {
    /// Retained installed codecs, including drained registrations.
    pub registry: &'a Registry,
    /// Local Services Recover authority for those retained codecs.
    pub recover: &'a services::contract::Context<'a>,
    /// Original durable owner read permission.
    pub observe: &'a services::contract::Context<'a>,
    /// Local retained operation owner.
    pub runtime: &'a Coordinator<P>,
}
/// Probe only eligible invoke proposals before consulting current Board terms or Ledger balances.
/// # Errors
/// Changed original fields conflict, unresolved ownership keys are unsupported, and uncertain original work stays Unknown.
pub fn provider<P: Persistence>(
    context: &Context<'_>,
    authority: &Authority,
    association: Association,
    message: &Message,
    ports: Lookup<'_, P>,
    limits: Limits,
) -> Result<Option<Cached>, Error> {
    tracing::trace!("Checking an original family cache before current invocation admission");
    context.check(
        authority,
        association.local,
        Action::Invoke,
        message.bytes().len(),
    )?;
    limits.validate()?;
    if association.local != ports.runtime.local_identity()
        || association.operation.requester != association.peer
        || association.operation.stream == 0
    {
        return Err(Error::Denied);
    }
    let v = message.value();
    if message.kind() != Kind::Propose
        || v["mode"].as_str() != Some("invoke")
        || v["service"].as_str() != Some("cr2se.messaging")
        || scalar::uint32(&v["serviceVersion"])? != 1
        || !matches!(v["input"]["operation"].as_str(), Some("recover" | "remove"))
    {
        return Ok(None);
    }
    let registration = match ports.registry.recover(ports.recover, "cr2se.messaging", 1) {
        Ok(pin) => pin,
        Err(services::Error::Unavailable) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let descriptor = &registration.requester().definition().document.value()["input"];
    let pending = Pending::parse(
        &v["input"],
        descriptor,
        limits,
        registration.requester().nonfinite(),
    )?;
    // Only defined ownership fields participate; ignored compatible extensions cannot change the replay identity.
    let input = pending.describe()?.complete(Budget {
        depth: 31,
        nodes: limits.nodes,
        bytes: limits.state_bytes,
    })?;
    let proposal = Proposal {
        service: "cr2se.messaging".into(),
        version: 1,
        offering: v["offeringId"].as_str().ok_or(Error::Invalid)?.into(),
        publisher: text::parse(v["publisherId"].as_str().ok_or(Error::Invalid)?)
            .map_err(|_| Error::Invalid)?,
        issuer: text::parse(v["creditIssuer"].as_str().ok_or(Error::Invalid)?)
            .map_err(|_| Error::Invalid)?,
        maximum: scalar::uint64(&v["maximumPrice"])?,
        payment: super::requester::payment(v)?,
        input,
    };
    ports
        .runtime
        .cached_provider(
            ports.observe,
            super::acceptance::peer(association),
            &proposal,
        )
        .map_err(Error::from)
}
/// Encode an explicitly authorized retry from original retained fields with no quote, new acceptance or new input body.
/// # Errors
/// Wrong local purpose, unsupported detached ownership fields and wire bounds fail without creating a new financial operation.
pub fn request(
    context: &Context<'_>,
    authority: &Authority,
    retry: &Retry,
    limits: Limits,
) -> Result<Message, Error> {
    tracing::trace!("Encoding an explicit original Messaging retransmission");
    let peer = retry.peer();
    context.check(authority, peer.local, Action::Request, 0)?;
    limits.validate()?;
    let invocation = retry.invocation();
    let descriptor = &invocation.definition.document.value()["input"];
    let encoded = value::encode(retry.input(), limits, true)?;
    let complete = Pending::parse(&encoded.metadata, descriptor, limits, true)?
        .describe()?
        .complete(Budget {
            depth: 31,
            nodes: limits.nodes,
            bytes: limits.state_bytes,
        })?;
    let encoded = value::encode(&complete, limits, true)?;
    if !encoded.bodies.is_empty() {
        return Err(Error::Unsupported);
    }
    let o = &invocation.offering;
    let mut wire = json!({"type":"propose","offeringId":o.id,"publisherId":text::format(invocation.publisher),"service":o.service,"serviceVersion":o.version,"mode":"invoke","input":encoded.metadata,"creditIssuer":text::format(invocation.issuer),"maximumPrice":retry.maximum().to_string()});
    if let (Some(id), Some(fee)) = (retry.payment().transfer, retry.payment().fee) {
        wire["transferId"] = json!(super::base64::encode(&id));
        wire["transferFee"] = json!(fee.to_string());
    }
    let bytes = serde_json::to_vec(&wire).map_err(|_| Error::Invalid)?;
    context.check(authority, peer.local, Action::Request, bytes.len())?;
    Message::parse(&bytes, limits.nodes)
}
/// Exact original cached output, associated with a new authenticated response stream and no new charge.
pub struct Sender {
    proof: Cached,
    sequence: Sequence,
}
impl Sender {
    /// Encode only owner/family-approved committed output using its original schema and price.
    /// # Errors
    /// Wrong authority, invalid retained price/output or finite encoding limits cannot produce a cached response.
    pub fn new(
        context: &Context<'_>,
        authority: &Authority,
        proof: Cached,
        limits: Limits,
    ) -> Result<Self, Error> {
        tracing::trace!("Creating an original cached response without requoting");
        let peer = proof.peer();
        context.check(authority, peer.local, Action::Invoke, 0)?;
        let completion = proof.completion();
        services::pricing::validate_output_price(&completion.output, completion.price)?;
        let encoded = value::encode(&completion.output, limits, true)?;
        let pending = Pending::parse(
            &encoded.metadata,
            &proof.invocation().definition.document.value()["output"],
            limits,
            true,
        )?;
        let bodies = Bodies::new(encoded.bodies, pending.descriptors(), limits)?;
        let wire=serde_json::to_vec(&json!({"type":"result","output":encoded.metadata,"finalPrice":completion.price.to_string()})).map_err(|_|Error::Invalid)?;
        Message::parse(&wire, limits.nodes)?;
        context.check(authority, peer.local, Action::Invoke, wire.len())?;
        Ok(Self {
            proof,
            sequence: Sequence::new(wire, bodies, b"{\"type\":\"result.end\"}"),
        })
    }
    /// Borrow the unforgeable original-owner proof for the exceptional cached-result ordering transition.
    pub fn proof(&self) -> &Cached {
        tracing::trace!("Borrowing original cached-response ordering proof");
        &self.proof
    }
    /// Queue cached result, bodies and result.end under bounded backpressure without any new quote/accept/input exchange.
    /// # Errors
    /// Queue refusal leaves progress unchanged and never affects the original committed result/payment.
    pub fn enqueue_next(&mut self, queue: &mut Outbound, chunk: usize) -> Result<bool, Error> {
        tracing::trace!("Queueing original cached response records");
        self.sequence.enqueue_next(queue, chunk)
    }
    /// Confirm all records are locally queued; composition drains and sends END before claiming normal completion.
    pub fn queued(&self) -> bool {
        tracing::trace!("Checking cached response queue completion");
        self.sequence.queued()
    }
}
