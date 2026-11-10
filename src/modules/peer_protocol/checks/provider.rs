//! Provider check quotes pin challenge-only ownership, exact economics and one original-stream acceptance.
use super::{state::State, *};
use crate::modules::peer_protocol::{
    acceptance::Admission,
    admission::Roles,
    base64,
    message::{Kind, Message},
    proposal::{Mode, Proposal},
};
use crate::modules::{
    identity::text,
    services::{self, handler::Blob, persistence::Persistence, quote::Current},
};
use serde_json::json;
use sha2::{Digest, Sha256};
/// Original provider check quote; no quote reserves credits or reads private expected bytes.
pub struct Issued {
    pub(super) state: State,
    generation: Arc<()>,
    domain: Weak<()>,
}
impl Issued {
    /// Borrow exact complete quote JSON, retained without reserialization.
    pub fn bytes(&self) -> &[u8] {
        tracing::trace!("Borrowing exact provider check quote");
        self.state.bytes.bytes()
    }
    /// Read the SHA256 binding to the exact original JSON body.
    pub fn hash(&self) -> [u8; 32] {
        tracing::trace!("Reading provider check quote hash");
        self.state.hash
    }
    /// Read original random quote identifier.
    pub fn id(&self) -> [u8; 32] {
        tracing::trace!("Reading provider check quote identifier");
        self.state.id
    }
    /// Read exact paid charge or zero included entitlement charge.
    pub fn price(&self) -> u64 {
        tracing::trace!("Reading provider check quote price");
        self.state.terms.price
    }
    /// Read original authenticated requester association.
    pub fn association(&self) -> Association {
        tracing::trace!("Reading provider check quote association");
        self.state.association
    }
}
impl Owner {
    /// Assess complete bounded challenge metadata and current check rules before issuing one finite exact quote.
    /// # Errors
    /// Local-only/absent rules, unresolved ownership keys, bad entitlement/payment, changed terms or entropy failure produces no hold or evidence.
    pub fn issue(
        &self,
        context: &Context<'_>,
        services_context: &services::contract::Context<'_>,
        proposal: Proposal,
        current: &dyn Current,
        expires: Instant,
    ) -> Result<Issued, Error> {
        tracing::trace!("Issuing an exact remote evidence-check quote");
        self.check(context, Action::Invoke, 0)?;
        if proposal.mode() != Mode::Check {
            return Err(Error::Unsupported);
        }
        if !proposal.input().descriptors().is_empty() {
            return Err(Error::Unsupported);
        }
        let challenge = proposal.input().assemble(&BTreeMap::new())?;
        services_context.check(
            &self.config.services,
            self.config.local,
            services::authority::Action::Invoke,
            challenge.byte_len(self.budget())?,
        )?;
        let authorized = proposal.authorized;
        let selection = authorized.selection();
        let (requester, provider, issuer) = selection.roles();
        if provider != self.config.local {
            return Err(Error::Denied);
        }
        current.check(selection)?;
        let rules = authorized
            .implementation()
            .requester()
            .checks()
            .ok_or(Error::Unsupported)?;
        let terms = services::checks::terms(rules, selection.offering(), &challenge)?;
        let _schema = super::evidence::schema(&selection.offering().service, rules)?;
        if terms
            .price
            .checked_add(proposal.payment.fee.unwrap_or(0))
            .ok_or(Error::Limit)?
            > proposal.maximum
        {
            return Err(Error::Insufficient);
        }
        if terms.price == 0 {
            if proposal.payment.transfer.is_some() || proposal.payment.fee.is_some() {
                return Err(Error::Invalid);
            }
        } else {
            self.config.gate.capacity(
                context,
                Roles {
                    requester,
                    provider,
                    issuer,
                },
                terms.price,
                proposal.payment.fee.unwrap_or(0),
            )?;
        }
        let expires = expires.min(context.deadline).min(services_context.deadline);
        if Instant::now() >= expires {
            return Err(Error::Cancelled);
        }
        let id = self.config.entropy.quote_id()?;
        let mut wire = json!({"type":"quote","quoteId":base64::encode(&id),"offering":selection.offering().source,"creditIssuer":text::format(issuer),"priceMode":"fixed","maximumPrice":terms.price.to_string(),"pricingInputs":{}});
        if let (Some(transfer), Some(fee)) = (proposal.payment.transfer, proposal.payment.fee) {
            wire["transferId"] = json!(base64::encode(&transfer));
            wire["transferFee"] = json!(fee.to_string());
        }
        let bytes = serde_json::to_vec(&wire).map_err(|_| Error::Invalid)?;
        Message::parse(&bytes, self.config.limits.nodes)?;
        self.check(context, Action::Invoke, bytes.len())?;
        let hash = Sha256::digest(&bytes).into();
        let association = self.association(&authorized);
        let state = State {
            authorized,
            challenge,
            bytes: Blob::new(bytes, self.config.limits.state_bytes)?,
            hash,
            id,
            terms,
            maximum: proposal.maximum,
            payment: proposal.payment,
            expires,
            association,
        };
        rules_for(&state)?.challenge(&state.invocation(), &state.challenge)?;
        current.check(state.authorized.selection())?;
        let generation = Arc::new(());
        let mut slots = self.slots.lock().map_err(|_| Error::Unavailable)?;
        slots.retain(|_, (_, end)| *end > Instant::now());
        if slots.contains_key(&state.authorized.key()) {
            return Err(Error::Conflict);
        }
        if slots.len() >= self.config.limits.quotes {
            return Err(Error::Limit);
        }
        slots.insert(state.authorized.key(), (generation.clone(), expires));
        Ok(Issued {
            state,
            generation,
            domain: Arc::downgrade(&self.domain),
        })
    }
    /// Release only the exact original unaccepted check generation, without changing admitted work.
    /// # Errors
    /// Wrong owner/purpose or stale generation cannot release another quote.
    pub fn discard(&self, context: &Context<'_>, issued: &Issued) -> Result<(), Error> {
        tracing::trace!("Releasing an original unaccepted check quote");
        self.check(context, Action::Invoke, 0)?;
        self.owns(&issued.domain)?;
        let mut slots = self.slots.lock().map_err(|_| Error::Unavailable)?;
        let key = issued.state.authorized.key();
        if let Some((generation, _)) = slots.get(&key) {
            if !Arc::ptr_eq(generation, &issued.generation) {
                return Err(Error::Conflict);
            }
            slots.remove(&key);
        }
        Ok(())
    }
    /// Reserve provider check capacity only after exact ID/hash and original stream acceptance.
    /// # Errors
    /// Invalid/stale acceptance cannot dispatch; durable uncertainty stays with the original operation and never implies rollback.
    pub fn accept<P: Persistence>(
        &self,
        context: &Context<'_>,
        dependencies: Admission<'_, P>,
        issued: Issued,
        observed: Association,
        message: &Message,
    ) -> Result<Incoming, Error> {
        tracing::trace!("Accepting and reserving original remote check terms");
        self.check(context, Action::Invoke, message.bytes().len())?;
        self.owns(&issued.domain)?;
        dependencies.quote_context.check(
            &self.config.services,
            self.config.local,
            services::authority::Action::Invoke,
            issued.bytes().len(),
        )?;
        let result = (|| -> Result<(), Error> {
            if observed != issued.association() || message.kind() != Kind::Accept {
                return Err(Error::Denied);
            }
            if Instant::now() >= issued.state.expires {
                return Err(Error::Cancelled);
            }
            let v = message.value();
            if base64::decode(v["quoteId"].as_str().ok_or(Error::Invalid)?, 32)? != issued.id()
                || base64::decode(v["quoteHash"].as_str().ok_or(Error::Invalid)?, 32)?
                    != issued.hash()
            {
                return Err(Error::Denied);
            }
            let mut slots = self.slots.lock().map_err(|_| Error::Unavailable)?;
            let (generation, _) = slots
                .get(&issued.state.authorized.key())
                .ok_or(Error::Conflict)?;
            if !Arc::ptr_eq(generation, &issued.generation) {
                return Err(Error::Conflict);
            }
            slots.remove(&issued.state.authorized.key());
            Ok(())
        })();
        if let Err(error) = result {
            self.discard(context, &issued)?;
            return Err(error);
        }
        let invocation = issued.state.invocation();
        let challenge = issued.state.challenge.clone();
        let schema =
            super::evidence::schema(&invocation.offering.service, rules_for(&issued.state)?)?;
        dependencies.runtime.admit_check(
            dependencies.coordinate_context,
            issued.state.agreement(),
            dependencies.current,
        )?;
        Ok(Incoming {
            association: observed,
            invocation,
            challenge,
            schema,
        })
    }
}
// Explicit installed rules remain pinned with the original selected implementation throughout quote validation.
fn rules_for(state: &State) -> Result<&dyn services::checks::Rules, Error> {
    tracing::trace!("Borrowing pinned installed check rules");
    state
        .authorized
        .implementation()
        .requester()
        .checks()
        .ok_or(Error::Unsupported)
}
