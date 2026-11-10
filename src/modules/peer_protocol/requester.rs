//! Requester-owned proposals, independent quote validation and explicit exact-byte spending consent.
use super::{
    Error,
    authority::{Action, Authority},
    base64,
    contract::{Association, Context, Limits},
    message::{Kind, Message},
    quote, scalar, value,
};
use crate::modules::{
    board::selection::Selection,
    identity::{api::IdentityId, text},
    services::{
        self,
        assessment::Assessed,
        handler::{Assessment, Blob, Peer},
        pricing::Price,
        registry::Registry,
        runtime::Payment,
        value::{Budget, Value},
    },
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    sync::{Arc, Weak},
    time::Instant,
};

/// Explicit local spending policy for one proposal; no implicit fee or automatic acceptance.
pub struct Spending {
    /// Total uint64 ceiling including any preagreed issuer fee.
    pub maximum: u64,
    /// Optional original third-party transfer and exact fee.
    pub payment: Payment,
}
/// Original authorized local input and selected terms; peer proposals cannot manufacture this capability.
pub struct Request {
    association: Association,
    selection: Selection,
    input: Value,
    maximum: u64,
    payment: Payment,
    wire: Blob,
    deadline: Instant,
    domain: Weak<()>,
}
impl Request {
    /// Borrow the exact proposal control; detached sources remain with the original caller until acceptance.
    pub fn bytes(&self) -> &[u8] {
        tracing::trace!("Borrowing authorized requester proposal bytes");
        self.wire.bytes()
    }
}
/// Independently checked candidate; merely receiving it never grants consent or reserves money.
pub struct Candidate {
    pub(crate) assessed: Assessed,
    pub(crate) bytes: Blob,
    pub(crate) id: [u8; 32],
    pub(crate) hash: [u8; 32],
    pub(crate) association: Association,
    pub(crate) maximum: u64,
    pub(crate) payment: Payment,
    pub(crate) price: Price,
    pub(crate) deadline: Instant,
    domain: Weak<()>,
}
impl Candidate {
    /// Borrow exact received quote bytes for inspection without reserialization.
    pub fn bytes(&self) -> &[u8] {
        tracing::trace!("Borrowing independently validated original quote bytes");
        self.bytes.bytes()
    }
    /// Read SHA256 of the exact received control body to identify the snapshot approved locally.
    pub fn hash(&self) -> [u8; 32] {
        tracing::trace!("Reading candidate exact quote hash");
        self.hash
    }
    /// Read the original random identifier for acceptance binding.
    pub fn id(&self) -> [u8; 32] {
        tracing::trace!("Reading validated provider quote identifier");
        self.id
    }
    /// Read independently calculated price authorization.
    pub fn price(&self) -> Price {
        tracing::trace!("Reading independently calculated requester quote price");
        self.price
    }
    /// Borrow validated complete offering and retrieved definition for a local consent decision.
    pub fn selection(&self) -> &Selection {
        tracing::trace!("Borrowing candidate contract for explicit local consent");
        self.assessed.authorized().selection()
    }
    /// Read original stream association for consent routing and later admission.
    pub fn association(&self) -> Association {
        tracing::trace!("Reading requester candidate association");
        self.association
    }
    /// Read the exact agreed transfer and issuer fee, still without commitment.
    pub fn payment(&self) -> Payment {
        tracing::trace!("Reading candidate issuer payment binding");
        self.payment
    }
    /// Read finite local deadline retained from the original proposal.
    pub fn deadline(&self) -> Instant {
        tracing::trace!("Reading requester quote deadline");
        self.deadline
    }
}
/// Explicitly approved candidate, still requiring one durable reservation before accept is sent.
pub struct Consented {
    pub(crate) candidate: Candidate,
}
impl Consented {
    /// Inspect the approved candidate; this is not permission to bypass admission.
    pub fn candidate(&self) -> &Candidate {
        tracing::trace!("Borrowing explicitly approved requester quote");
        &self.candidate
    }
}
/// Local requester wire owner; registry capabilities are explicitly installed, never supplied by a peer.
pub struct Client {
    pub(crate) local: IdentityId,
    pub(crate) authority: Arc<Authority>,
    pub(crate) services: Arc<services::authority::Authority>,
    registry: Arc<Registry>,
    limits: Limits,
    pub(crate) service_limits: services::contract::Limits,
    domain: Arc<()>,
}
impl Client {
    /// Configure independent requester authority and real family codecs without installing providers.
    /// # Errors
    /// Invalid limits or a foreign Registry are rejected.
    pub fn new(
        local: IdentityId,
        authority: Arc<Authority>,
        services: Arc<services::authority::Authority>,
        registry: Arc<Registry>,
        limits: Limits,
        service_limits: services::contract::Limits,
    ) -> Result<Self, Error> {
        tracing::trace!("Creating requester quote validation owner");
        limits.validate()?;
        service_limits.validate()?;
        if registry.local_identity() != local {
            return Err(Error::Denied);
        }
        Ok(Self {
            local,
            authority,
            services,
            registry,
            limits,
            service_limits,
            domain: Arc::new(()),
        })
    }
    /// Build an ordinary proposal only from authorized local input and explicit spending/payment policy.
    /// # Errors
    /// Wrong roles, unsupported contract, malformed input or payment, zero maximum and bounds fail without effects.
    pub fn request(
        &self,
        context: &Context<'_>,
        services_context: &services::contract::Context<'_>,
        association: Association,
        selection: Selection,
        input: Value,
        spending: Spending,
    ) -> Result<Request, Error> {
        tracing::trace!("Building locally authorized requester proposal");
        let Spending { maximum, payment } = spending;
        context.check(&self.authority, self.local, Action::Request, 0)?;
        let (requester, provider, issuer) = selection.roles();
        if association.local != self.local
            || requester != self.local
            || provider != association.peer
            || association.operation.requester != requester
            || maximum == 0
        {
            return Err(Error::Denied);
        }
        check_payment(payment, requester, provider, issuer)?;
        let authorized = services::authorization::authorize(
            services_context,
            &self.services,
            self.local,
            &self.registry,
            selection.clone(),
            peer(association),
            Some(input.clone()),
        )?;
        let implementation = authorized.implementation().requester();
        services::validation::complete(
            &input,
            &selection.definition().document.value()["input"],
            self.budget(),
            implementation,
        )?;
        let encoded = value::encode(&input, self.limits, implementation.nonfinite())?;
        let o = selection.offering();
        let mut wire = json!({"type":"propose","offeringId":o.id,"publisherId":text::format(selection.publisher()),"service":o.service,"serviceVersion":o.version,"mode":"invoke","input":encoded.metadata,"creditIssuer":text::format(issuer),"maximumPrice":maximum.to_string()});
        if let (Some(id), Some(fee)) = (payment.transfer, payment.fee) {
            wire["transferId"] = json!(base64::encode(&id));
            wire["transferFee"] = json!(fee.to_string());
        }
        let bytes = serde_json::to_vec(&wire).map_err(|_| Error::Invalid)?;
        Message::parse(&bytes, self.limits.nodes)?;
        context.check(&self.authority, self.local, Action::Request, bytes.len())?;
        Ok(Request {
            association,
            selection,
            input,
            maximum,
            payment,
            wire: Blob::new(bytes, self.limits.state_bytes)?,
            deadline: context.deadline.min(services_context.deadline),
            domain: Arc::downgrade(&self.domain),
        })
    }
    /// Attribute an incoming quote to the original request and independently recalculate family economics.
    /// # Errors
    /// Foreign/stale requests, changed contracts/issuer, invalid factors, price/fee mismatches and unsupported models fail before consent.
    pub fn validate(
        &self,
        context: &Context<'_>,
        services_context: &services::contract::Context<'_>,
        request: Request,
        observed: Association,
        message: &Message,
    ) -> Result<Candidate, Error> {
        tracing::trace!("Independently validating authenticated provider quote");
        context.check(
            &self.authority,
            self.local,
            Action::Invoke,
            message.bytes().len(),
        )?;
        self.owns(&request.domain)?;
        if observed != request.association || message.kind() != Kind::Quote {
            return Err(Error::Denied);
        }
        if Instant::now() >= request.deadline {
            return Err(Error::Cancelled);
        }
        let v = message.value();
        let selected = request.selection.quote_terms(
            &v["offering"],
            self.registry.as_ref(),
            Default::default(),
        )?;
        let (requester, provider, issuer) = selected.roles();
        if issuer != request.selection.roles().2
            || text::parse(v["creditIssuer"].as_str().ok_or(Error::Invalid)?)
                .map_err(|_| Error::Invalid)?
                != issuer
        {
            return Err(Error::Denied);
        }
        check_payment(request.payment, requester, provider, issuer)?;
        let payment = payment(v)?;
        if payment.transfer != request.payment.transfer || payment.fee != request.payment.fee {
            return Err(Error::Denied);
        }
        let authorized = services::authorization::authorize(
            services_context,
            &self.services,
            self.local,
            &self.registry,
            selected,
            peer(observed),
            Some(request.input),
        )?;
        let input = authorized.input().ok_or(Error::Denied)?;
        let metadata = authorized
            .implementation()
            .requester()
            .assessment_metadata(input, self.budget())?;
        let described = services::input::Input::Complete(input.clone());
        let operation = quote::operation(&described);
        let factors = factors(&authorized.selection().offering().service, operation, v)?;
        let input_bytes = input.byte_len(self.budget())? as u64;
        let assessed = services::assessment::remote(
            services_context,
            &self.services,
            authorized,
            Assessment {
                metadata,
                factors,
                input_bytes,
                pin: None,
            },
            self.service_limits,
        )?;
        let price = services::pricing::quote(&assessed, request.maximum)?;
        let offering = assessed.authorized().selection().offering();
        let maximum = price.maximum(&offering.service, offering.version)?;
        let mode = if matches!(price, Price::Computation { .. }) {
            "metered"
        } else {
            "fixed"
        };
        if v["priceMode"].as_str() != Some(mode) || scalar::uint64(&v["maximumPrice"])? != maximum {
            return Err(Error::Invalid);
        }
        if maximum
            .checked_add(payment.fee.unwrap_or(0))
            .ok_or(Error::Limit)?
            > request.maximum
        {
            return Err(Error::Insufficient);
        }
        let bytes = Blob::new(message.bytes().to_vec(), self.limits.state_bytes)?;
        let hash = Sha256::digest(bytes.bytes()).into();
        let id = base64::decode(v["quoteId"].as_str().ok_or(Error::Invalid)?, 32)?
            .try_into()
            .map_err(|_| Error::Invalid)?;
        Ok(Candidate {
            assessed,
            bytes,
            id,
            hash,
            association: observed,
            maximum: request.maximum,
            payment,
            price,
            deadline: request.deadline,
            domain: Arc::downgrade(&self.domain),
        })
    }
    /// Approve exactly the locally inspected candidate hash with a fresh explicit total spending ceiling.
    /// # Errors
    /// Wrong owner/hash, expired context or price plus fee above either original or consent ceiling fails without reservation.
    pub fn consent(
        &self,
        context: &Context<'_>,
        candidate: Candidate,
        hash: [u8; 32],
        total_maximum: u64,
    ) -> Result<Consented, Error> {
        tracing::trace!("Recording explicit local consent to exact quoted snapshot");
        context.check(
            &self.authority,
            self.local,
            Action::Request,
            candidate.bytes.bytes().len(),
        )?;
        self.owns(&candidate.domain)?;
        if candidate.hash != hash {
            return Err(Error::Denied);
        }
        if Instant::now() >= candidate.deadline {
            return Err(Error::Cancelled);
        }
        let o = candidate.selection().offering();
        let total = candidate
            .price
            .maximum(&o.service, o.version)?
            .checked_add(candidate.payment.fee.unwrap_or(0))
            .ok_or(Error::Limit)?;
        if total > total_maximum || total > candidate.maximum {
            return Err(Error::Insufficient);
        }
        Ok(Consented { candidate })
    }
    /// Reserve this owner's explicitly consented candidate before any accept bytes can be sent.
    /// # Errors
    /// Foreign consent, changed terms, failed reservation and unknown persistence outcomes preserve original-key recovery obligations.
    pub fn reserve<P: services::persistence::Persistence>(
        &self,
        context: &Context<'_>,
        dependencies: super::acceptance::Admission<'_, P>,
        consent: Consented,
    ) -> Result<super::acceptance::Outgoing, Error> {
        tracing::trace!("Admitting explicitly approved requester quote");
        self.owns(&consent.candidate.domain)?;
        super::acceptance::reserve(self, context, dependencies, consent)
    }
    // Each owner has an independent opaque domain; public quote fields cannot forge local input/consent.
    fn owns(&self, weak: &Weak<()>) -> Result<(), Error> {
        tracing::trace!("Checking requester quote owner provenance");
        if !Arc::ptr_eq(&weak.upgrade().ok_or(Error::Denied)?, &self.domain) {
            return Err(Error::Denied);
        }
        Ok(())
    }
    // Both service validation and remote metadata use the same explicit finite logical-value budget.
    fn budget(&self) -> Budget {
        tracing::trace!("Reading finite requester validation budget");
        Budget {
            depth: 31,
            nodes: self.service_limits.nodes,
            bytes: self.service_limits.state_bytes,
        }
    }
}
// Construct the Services context only from the trusted adapter association, never from a quote's identity fields.
fn peer(a: Association) -> Peer {
    tracing::trace!("Mapping trusted requester stream association to Services");
    Peer {
        local: a.local,
        remote: a.peer,
        key: services::contract::OperationKey {
            requester: a.operation.requester,
            connection: a.operation.connection,
            stream: a.operation.stream,
        },
    }
}
// Require the complete third-party tuple and forbid unsolicited participating-issuer payment fields.
fn check_payment(
    payment: Payment,
    requester: IdentityId,
    provider: IdentityId,
    issuer: IdentityId,
) -> Result<(), Error> {
    tracing::trace!("Checking explicit requester issuer payment choice");
    if (issuer != requester && issuer != provider)
        != (payment.transfer.is_some() && payment.fee.is_some())
        || payment.transfer.is_some() != payment.fee.is_some()
    {
        return Err(Error::Invalid);
    }
    Ok(())
}
// Decode only the original optional paired transfer values already validated as canonical control fields.
fn payment(v: &serde_json::Value) -> Result<Payment, Error> {
    tracing::trace!("Decoding quote transfer and exact fee binding");
    match (v.get("transferId"), v.get("transferFee")) {
        (None, None) => Ok(Payment::default()),
        (Some(id), Some(fee)) => Ok(Payment {
            transfer: Some(
                base64::decode(id.as_str().ok_or(Error::Invalid)?, 16)?
                    .try_into()
                    .map_err(|_| Error::Invalid)?,
            ),
            fee: Some(scalar::uint64(fee)?),
        }),
        _ => Err(Error::Invalid),
    }
}
// Reconstruct the narrow family factors and independently verify the specified wire projection before family validation.
fn factors(
    service: &str,
    operation: Option<&str>,
    wire: &serde_json::Value,
) -> Result<Value, Error> {
    tracing::trace!("Checking specified provider pricing input fields");
    let map = wire["pricingInputs"].as_object().ok_or(Error::Invalid)?;
    let mut fields = Vec::with_capacity(map.len());
    for (name, value) in map {
        fields.push((name.clone(), Value::Uint64(scalar::uint64(value)?)));
    }
    if service == "cr2se.messaging" && operation == Some("remove") {
        if !fields.is_empty() {
            return Err(Error::Invalid);
        }
        fields.push((
            "releaseCredits".into(),
            Value::Uint64(scalar::uint64(&wire["releaseCredits"])?),
        ));
    } else if wire.get("releaseCredits").is_some() {
        return Err(Error::Invalid);
    }
    let factors = Value::Object(fields);
    quote::factors(service, operation, &factors)?;
    Ok(factors)
}
