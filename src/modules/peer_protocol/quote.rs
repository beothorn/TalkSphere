//! Exact provider quote construction over bounded metadata, independent price rules and original-stream Services handles.
use super::{
    Error,
    admission::{Gate, Roles},
    authority::{Action, Authority},
    base64,
    contract::{Association, Context, Limits, OperationKey},
    message::Message,
    proposal::{Mode, Proposal},
    value::Pending,
};
use crate::modules::{
    identity::{api::IdentityId, text},
    services::{
        self,
        handler::{Blob, Peer},
        input::Input,
        pricing::Price,
        quote::{Current, Issue, Quote, Quoter},
        runtime::Payment,
        value::Value,
    },
};
use serde_json::{Value as Json, json};
use std::{
    sync::{Arc, Weak},
    time::Instant,
};
/// CSPRNG port supplied independently from wire parsing and application payloads.
pub trait Entropy: Send + Sync {
    /// Return32 fresh random quote identifier bytes or a typed dependency failure.
    fn quote_id(&self) -> Result<[u8; 32], Error>;
}
/// Production system CSPRNG; failure never becomes a predictable identifier.
pub struct SystemEntropy;
impl Entropy for SystemEntropy {
    // Randomness comes from the OS, not from metadata hashes, clocks or sequential request IDs.
    fn quote_id(&self) -> Result<[u8; 32], Error> {
        tracing::trace!("Generating a fresh peer quote identifier");
        let mut bytes = [0; 32];
        getrandom::fill(&mut bytes).map_err(|_| Error::Unavailable)?;
        Ok(bytes)
    }
}
/// Opaque issued quote on one receiving owner and stream; it contains no accepted work or reservation.
pub struct Issued {
    pub(crate) service: Quote,
    pub(crate) input: Pending,
    pub(crate) payment: Payment,
    pub(crate) association: Association,
    pub(crate) price: Price,
    pub(crate) expires: Instant,
    id: [u8; 32],
    domain: Weak<()>,
}
impl Issued {
    /// Borrow exact JSON body bytes used for hashing and transmission.
    pub fn bytes(&self) -> &[u8] {
        tracing::trace!("Borrowing immutable peer quote JSON");
        self.service.bytes()
    }
    /// Read the exact original quote hash, never a reserialized equivalent.
    pub fn hash(&self) -> [u8; 32] {
        tracing::trace!("Reading exact issued peer quote hash");
        self.service.hash()
    }
    /// Read the original random32-byte identifier.
    pub fn id(&self) -> [u8; 32] {
        tracing::trace!("Reading issued peer quote identifier");
        self.id
    }
    /// Read original authenticated endpoint and operation correlation.
    pub fn association(&self) -> Association {
        tracing::trace!("Reading original quote stream association");
        self.association
    }
    /// Read immutable accepted-price candidate, still requiring explicit consent/reservation.
    pub fn price(&self) -> Price {
        tracing::trace!("Reading issued peer price authorization candidate");
        self.price
    }
    /// Read bounded provisional input declarations, without successful complete content.
    pub fn input(&self) -> &Pending {
        tracing::trace!("Borrowing issued quote input declarations");
        &self.input
    }
    /// Read the exact original optional transfer and fee.
    pub fn payment(&self) -> Payment {
        tracing::trace!("Reading issued quote payment candidate");
        self.payment
    }
    /// Read local expiry; it is never a peer clock value.
    pub fn expires(&self) -> Instant {
        tracing::trace!("Reading local issued quote expiry");
        self.expires
    }
}
/// Provider quote owner retains separate authority domains and finite Services quote capacity.
pub struct Issuer {
    pub(crate) local: IdentityId,
    pub(crate) authority: Arc<Authority>,
    pub(crate) services: Arc<services::authority::Authority>,
    pub(crate) quoter: Quoter,
    limits: Limits,
    service_limits: services::contract::Limits,
    gate: Arc<dyn Gate>,
    entropy: Arc<dyn Entropy>,
    domain: Arc<()>,
}
impl Issuer {
    /// Configure explicit trusted economic/entropy ports without registering networking or installing handlers.
    /// # Errors
    /// Invalid limits fail before a quote owner exists.
    pub fn new(
        local: IdentityId,
        authority: Arc<Authority>,
        services: Arc<services::authority::Authority>,
        limits: Limits,
        service_limits: services::contract::Limits,
        gate: Arc<dyn Gate>,
        entropy: Arc<dyn Entropy>,
    ) -> Result<Self, Error> {
        tracing::trace!("Creating bounded provider peer quote owner");
        limits.validate()?;
        service_limits.validate()?;
        let quoter = Quoter::new(local, services.clone(), service_limits)?;
        Ok(Self {
            local,
            authority,
            services,
            quoter,
            limits,
            service_limits,
            gate,
            entropy,
            domain: Arc::new(()),
        })
    }
    /// Assess declared metadata and encode one immutable current quote before any credit reservation or execution.
    /// # Errors
    /// Wrong purpose/context, stale terms, unsupported factors, price/fee/capacity or entropy failure produces no issued quote.
    pub fn issue(
        &self,
        context: &Context<'_>,
        services_context: &services::contract::Context<'_>,
        proposal: Proposal,
        current: &dyn Current,
        expires: Instant,
    ) -> Result<Issued, Error> {
        tracing::trace!("Constructing an exact current provider quote");
        context.check(&self.authority, self.local, Action::Invoke, 0)?;
        if proposal.mode != Mode::Invoke {
            return Err(Error::Unsupported);
        }
        let expires = expires.min(context.deadline).min(services_context.deadline);
        if Instant::now() >= expires {
            return Err(Error::Cancelled);
        }
        let selection = proposal.authorized.selection().clone();
        let (requester, provider, issuer) = selection.roles();
        if provider != self.local {
            return Err(Error::Denied);
        }
        current.check(&selection)?;
        let metadata = proposal.input.describe()?;
        let operation = operation(&metadata);
        let assessed = services::assessment::assess_metadata(
            services_context,
            &self.services,
            proposal.authorized,
            &metadata,
            self.service_limits,
        )?;
        let price = services::pricing::quote(&assessed, proposal.maximum)?;
        let maximum = price.maximum(&selection.offering().service, selection.offering().version)?;
        let fee = proposal.payment.fee.unwrap_or(0);
        if maximum.checked_add(fee).ok_or(Error::Limit)? > proposal.maximum {
            return Err(Error::Insufficient);
        }
        self.gate.capacity(
            context,
            Roles {
                requester,
                provider,
                issuer,
            },
            maximum,
            fee,
        )?;
        let (pricing_inputs, release) =
            factors(&selection.offering().service, operation, assessed.factors())?;
        current.check(&selection)?;
        let id = self.entropy.quote_id()?;
        let mut wire = json!({"type":"quote","quoteId":base64::encode(&id),"offering":selection.offering().source,"creditIssuer":text::format(issuer),"priceMode":if matches!(price,Price::Computation{..}){"metered"}else{"fixed"},"maximumPrice":maximum.to_string(),"pricingInputs":pricing_inputs});
        if let Some(release) = release {
            wire["releaseCredits"] = json!(release.to_string());
        }
        if let (Some(transfer), Some(fee)) = (proposal.payment.transfer, proposal.payment.fee) {
            wire["transferId"] = json!(base64::encode(&transfer));
            wire["transferFee"] = json!(fee.to_string());
        }
        let bytes = serde_json::to_vec(&wire).map_err(|_| Error::Invalid)?;
        Message::parse(&bytes, self.limits.nodes)?;
        context.check(&self.authority, self.local, Action::Invoke, bytes.len())?;
        let key = assessed.authorized().key();
        let association = Association {
            local: self.local,
            peer: requester,
            operation: OperationKey {
                requester: key.requester,
                connection: key.connection,
                stream: key.stream,
            },
        };
        let service = self.quoter.issue(
            services_context,
            Issue {
                assessed,
                bytes: Blob::new(bytes, self.limits.state_bytes)?,
                maximum: proposal.maximum,
                expires,
            },
        )?;
        if service.price() != price {
            self.quoter.discard(
                services_context,
                &service,
                Peer {
                    local: self.local,
                    remote: requester,
                    key,
                },
            )?;
            return Err(Error::Conflict);
        }
        Ok(Issued {
            service,
            input: proposal.input,
            payment: proposal.payment,
            association,
            price,
            expires,
            id,
            domain: Arc::downgrade(&self.domain),
        })
    }
    /// Release one unaccepted quote after a known local disconnect/cancellation, without undoing accepted work.
    /// # Errors
    /// Foreign owner tokens and scoped authority failures cannot release another quote.
    pub fn discard(
        &self,
        context: &Context<'_>,
        services_context: &services::contract::Context<'_>,
        issued: &Issued,
    ) -> Result<(), Error> {
        tracing::trace!("Releasing one unaccepted peer quote");
        context.check(&self.authority, self.local, Action::Invoke, 0)?;
        self.owns(issued)?;
        let key = issued.association.operation;
        self.quoter
            .discard(
                services_context,
                &issued.service,
                Peer {
                    local: self.local,
                    remote: issued.association.peer,
                    key: services::contract::OperationKey {
                        requester: key.requester,
                        connection: key.connection,
                        stream: key.stream,
                    },
                },
            )
            .map_err(Error::from)
    }
    // The opaque issuer token cannot be transplanted across owners even when all public values match.
    pub(crate) fn owns(&self, issued: &Issued) -> Result<(), Error> {
        tracing::trace!("Checking exact peer quote issuer provenance");
        let domain = issued.domain.upgrade().ok_or(Error::Denied)?;
        if !Arc::ptr_eq(&domain, &self.domain) {
            return Err(Error::Denied);
        }
        Ok(())
    }
}
// Read only the already validated operation discriminant; unresolved bytes cannot invent an operation.
pub(crate) fn operation(metadata: &Input) -> Option<&str> {
    tracing::trace!("Reading validated proposal operation discriminant");
    match metadata {
        Input::Complete(Value::Object(fields)) => fields
            .iter()
            .find(|(n, _)| n == "operation")
            .and_then(|(_, v)| match v {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            }),
        Input::Object(fields) => {
            fields
                .iter()
                .find(|(n, _)| n == "operation")
                .and_then(|(_, v)| match v {
                    Input::Complete(Value::String(s)) => Some(s.as_str()),
                    _ => None,
                })
        }
        _ => None,
    }
}
// Only specified provider-dependent factors enter version-one pricingInputs; Messaging removal binds release separately.
pub(crate) fn factors(
    service: &str,
    operation: Option<&str>,
    factors: &Value,
) -> Result<(Json, Option<u64>), Error> {
    tracing::trace!("Binding only specified provider-dependent quote inputs");
    let Value::Object(fields) = factors else {
        return Err(Error::Invalid);
    };
    let expected = match (service, operation) {
        ("cr2se.public-file-sharing", Some("manifest")) => Some("manifestBytes"),
        ("cr2se.public-file-sharing", Some("download")) => Some("downloadBytes"),
        ("cr2se.storage", Some("renew")) => Some("sizeBytes"),
        ("cr2se.messaging", Some("recover" | "renew")) => Some("payloadSize"),
        _ => None,
    };
    if service == "cr2se.messaging" && operation == Some("remove") {
        if let [(name, Value::Uint64(n))] = fields.as_slice()
            && name == "releaseCredits"
        {
            return Ok((json!({}), Some(*n)));
        }
        return Err(Error::Invalid);
    }
    if let Some(expected) = expected {
        if let [(name, Value::Uint64(n))] = fields.as_slice()
            && name == expected
        {
            return Ok((json!({expected:n.to_string()}), None));
        }
        return Err(Error::Invalid);
    }
    if !fields.is_empty() {
        return Err(Error::Unsupported);
    }
    Ok((json!({}), None))
}
