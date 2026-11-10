//! Requester-owned challenge proposals and explicit check consent preserve original input and exact quote bytes.
use super::{state::State, *};
use crate::modules::peer_protocol::{
    acceptance::{Admission, peer},
    base64,
    message::{Kind, Message},
    requester::Spending,
    scalar, value,
};
use crate::modules::{
    identity::text,
    services::{self, authorization::Authorized, checks, handler::Blob, persistence::Persistence},
};
use serde_json::json;
use sha2::{Digest, Sha256};
/// Original locally authorized challenge; the API accepts no private expected byte or provider outcome.
pub struct Request {
    authorized: Authorized,
    challenge: services::value::Value,
    spending: Spending,
    bytes: Blob,
    expires: Instant,
    domain: Weak<()>,
}
impl Request {
    /// Borrow original challenge-only proposal JSON; complete local expectations remain outside this type.
    pub fn bytes(&self) -> &[u8] {
        tracing::trace!("Borrowing local challenge-only proposal");
        self.bytes.bytes()
    }
}
/// Independently calculated remote check snapshot, still without consent or financial reservation.
pub struct Candidate {
    state: State,
    domain: Weak<()>,
}
impl Candidate {
    /// Borrow exact original received quote JSON.
    pub fn bytes(&self) -> &[u8] {
        tracing::trace!("Borrowing validated check candidate bytes");
        self.state.bytes.bytes()
    }
    /// Read SHA256 of exact received quote bytes for local approval.
    pub fn hash(&self) -> [u8; 32] {
        tracing::trace!("Reading validated check candidate hash");
        self.state.hash
    }
    /// Read independently calculated paid price or included zero charge.
    pub fn price(&self) -> u64 {
        tracing::trace!("Reading independently calculated check price");
        self.state.terms.price
    }
}
/// Opaque local approval of one exact check quote and total spending ceiling.
pub struct Consented {
    state: State,
    domain: Weak<()>,
}
impl Owner {
    /// Build a check proposal from locally authorized challenge-only input and explicit spending policy.
    /// # Errors
    /// Local-only/uninstalled rules, wrong roles, bad challenge or payment and unsupported detached ownership keys fail without spending.
    pub fn request(
        &self,
        context: &Context<'_>,
        services_context: &services::contract::Context<'_>,
        association: Association,
        selection: crate::modules::board::selection::Selection,
        challenge: services::value::Value,
        spending: Spending,
    ) -> Result<Request, Error> {
        tracing::trace!("Building an authorized requester evidence challenge");
        self.check(context, Action::Request, 0)?;
        let (requester, provider, issuer) = selection.roles();
        if association.local != self.config.local
            || requester != self.config.local
            || provider != association.peer
            || association.operation.requester != requester
        {
            return Err(Error::Denied);
        }
        let authorized = services::authorization::authorize(
            services_context,
            &self.config.services,
            self.config.local,
            &self.config.registry,
            selection,
            peer(association),
            Some(challenge.clone()),
        )?;
        let rules = authorized
            .implementation()
            .requester()
            .checks()
            .ok_or(Error::Unsupported)?;
        let terms = checks::terms(rules, authorized.selection().offering(), &challenge)?;
        super::evidence::schema(&authorized.selection().offering().service, rules)?;
        services::validation::validate(
            &challenge,
            &authorized.selection().definition().document.value()["check"]["input"],
            self.budget(),
            false,
        )?;
        if terms.price == 0 {
            if spending.payment.transfer.is_some() || spending.payment.fee.is_some() {
                return Err(Error::Invalid);
            }
        } else {
            super::super::requester::check_payment(spending.payment, requester, provider, issuer)?;
        }
        if terms
            .price
            .checked_add(spending.payment.fee.unwrap_or(0))
            .ok_or(Error::Limit)?
            > spending.maximum
        {
            return Err(Error::Insufficient);
        }
        let encoded = value::encode(&challenge, self.config.limits, false)?;
        if !encoded.bodies.is_empty() {
            return Err(Error::Unsupported);
        }
        let selected = authorized.selection();
        let o = selected.offering();
        let mut wire = json!({"type":"propose","offeringId":o.id,"publisherId":text::format(selected.publisher()),"service":o.service,"serviceVersion":o.version,"mode":"check","input":encoded.metadata,"creditIssuer":text::format(issuer),"maximumPrice":spending.maximum.to_string()});
        if let (Some(id), Some(fee)) = (spending.payment.transfer, spending.payment.fee) {
            wire["transferId"] = json!(base64::encode(&id));
            wire["transferFee"] = json!(fee.to_string());
        }
        let bytes = serde_json::to_vec(&wire).map_err(|_| Error::Invalid)?;
        Message::parse(&bytes, self.config.limits.nodes)?;
        self.check(context, Action::Request, bytes.len())?;
        Ok(Request {
            authorized,
            challenge,
            spending,
            bytes: Blob::new(bytes, self.config.limits.state_bytes)?,
            expires: context.deadline.min(services_context.deadline),
            domain: Arc::downgrade(&self.domain),
        })
    }
    /// Attribute and recalculate a check quote against the original challenge, contract and explicit issuer/payment choice.
    /// # Errors
    /// Changed contract/issuer/stream, fabricated factors/outcomes, mismatched exact price and unsupported rules cannot become approval.
    pub fn validate(
        &self,
        context: &Context<'_>,
        services_context: &services::contract::Context<'_>,
        request: Request,
        observed: Association,
        message: &Message,
    ) -> Result<Candidate, Error> {
        tracing::trace!("Independently validating an original provider check quote");
        self.check(context, Action::Invoke, message.bytes().len())?;
        self.owns(&request.domain)?;
        if observed != self.association(&request.authorized) || message.kind() != Kind::Quote {
            return Err(Error::Denied);
        }
        if Instant::now() >= request.expires {
            return Err(Error::Cancelled);
        }
        let v = message.value();
        let old = request.authorized.selection();
        let selection = old.quote_terms(
            &v["offering"],
            self.config.registry.as_ref(),
            Default::default(),
        )?;
        let issuer = selection.roles().2;
        if issuer != old.roles().2
            || text::parse(v["creditIssuer"].as_str().ok_or(Error::Invalid)?)
                .map_err(|_| Error::Invalid)?
                != issuer
        {
            return Err(Error::Denied);
        }
        let payment = super::super::requester::payment(v)?;
        if payment.transfer != request.spending.payment.transfer
            || payment.fee != request.spending.payment.fee
        {
            return Err(Error::Denied);
        }
        if v["priceMode"].as_str() != Some("fixed")
            || v["pricingInputs"] != json!({})
            || v.get("releaseCredits").is_some()
        {
            return Err(Error::Invalid);
        }
        let authorized = services::authorization::authorize(
            services_context,
            &self.config.services,
            self.config.local,
            &self.config.registry,
            selection,
            peer(observed),
            Some(request.challenge.clone()),
        )?;
        let rules = authorized
            .implementation()
            .requester()
            .checks()
            .ok_or(Error::Unsupported)?;
        let terms = checks::terms(rules, authorized.selection().offering(), &request.challenge)?;
        super::evidence::schema(&authorized.selection().offering().service, rules)?;
        if scalar::uint64(&v["maximumPrice"])? != terms.price {
            return Err(Error::Invalid);
        }
        if terms
            .price
            .checked_add(payment.fee.unwrap_or(0))
            .ok_or(Error::Limit)?
            > request.spending.maximum
        {
            return Err(Error::Insufficient);
        }
        let bytes = Blob::new(message.bytes().to_vec(), self.config.limits.state_bytes)?;
        let hash = Sha256::digest(bytes.bytes()).into();
        let id = base64::decode(v["quoteId"].as_str().ok_or(Error::Invalid)?, 32)?
            .try_into()
            .map_err(|_| Error::Invalid)?;
        let state = State {
            authorized,
            challenge: request.challenge,
            bytes,
            hash,
            id,
            terms,
            maximum: request.spending.maximum,
            payment,
            expires: request.expires,
            association: observed,
        };
        Ok(Candidate {
            state,
            domain: Arc::downgrade(&self.domain),
        })
    }
    /// Explicitly approve the exact candidate hash under both original and current total spending ceilings.
    /// # Errors
    /// Foreign candidate, changed hash, expired context and insufficient explicit ceiling cannot reserve money.
    pub fn consent(
        &self,
        context: &Context<'_>,
        candidate: Candidate,
        hash: [u8; 32],
        maximum: u64,
    ) -> Result<Consented, Error> {
        tracing::trace!("Recording explicit local approval of exact check terms");
        self.check(context, Action::Request, candidate.bytes().len())?;
        self.owns(&candidate.domain)?;
        if hash != candidate.hash() {
            return Err(Error::Denied);
        }
        if Instant::now() >= candidate.state.expires {
            return Err(Error::Cancelled);
        }
        let total = candidate
            .state
            .terms
            .price
            .checked_add(candidate.state.payment.fee.unwrap_or(0))
            .ok_or(Error::Limit)?;
        if total > maximum || total > candidate.state.maximum {
            return Err(Error::Insufficient);
        }
        Ok(Consented {
            state: candidate.state,
            domain: Arc::downgrade(&self.domain),
        })
    }
    /// Reserve and mark one requester check dispatch before returning any accept/challenge terminal bytes.
    /// # Errors
    /// Wrong authority, changed immutable check rules, failed reservation and durable ambiguity never authorize a second dispatch.
    pub fn reserve<P: Persistence>(
        &self,
        context: &Context<'_>,
        dependencies: Admission<'_, P>,
        consent: Consented,
    ) -> Result<Outgoing, Error> {
        tracing::trace!("Reserving one explicit requester check before wire acceptance");
        self.check(context, Action::Request, consent.state.bytes.bytes().len())?;
        self.owns(&consent.domain)?;
        if Instant::now() >= consent.state.expires {
            return Err(Error::Cancelled);
        }
        dependencies.quote_context.check(
            &self.config.services,
            self.config.local,
            services::authority::Action::Request,
            consent.state.bytes.bytes().len(),
        )?;
        let association = consent.state.association;
        let invocation = consent.state.invocation();
        let challenge = consent.state.challenge.clone();
        let schema = super::evidence::schema(
            &invocation.offering.service,
            consent
                .state
                .authorized
                .implementation()
                .requester()
                .checks()
                .ok_or(Error::Unsupported)?,
        )?;
        let accept=serde_json::to_vec(&json!({"type":"accept","quoteId":base64::encode(&consent.state.id),"quoteHash":base64::encode(&consent.state.hash)})).map_err(|_|Error::Invalid)?;
        let key = dependencies.runtime.admit_check(
            dependencies.coordinate_context,
            consent.state.agreement(),
            dependencies.current,
        )?;
        dependencies.runtime.supply_input(
            dependencies.coordinate_context,
            key,
            challenge.clone(),
        )?;
        dependencies
            .runtime
            .outbound_check(dependencies.coordinate_context, key)?;
        Ok(Outgoing {
            association,
            invocation,
            challenge,
            accept,
            schema,
        })
    }
}
