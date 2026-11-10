//! Wanted-offering invitations reverse application roles while keeping input and spending under local requester policy.
use super::{
    Error,
    admission::Gate,
    authority::{Action, Authority},
    contract::{Association, Context, Limits},
    message::{Kind, Message},
    proposal::{Authorizer, Proposal},
    requester::{Client, Request, Spending},
};
use crate::modules::{
    board::{offering::Direction, selection::Selection},
    identity::api::IdentityId,
    services::{self, quote::Current, registry::Registry, value::Value},
};
use serde_json::json;
use std::{
    sync::{Arc, Weak},
    time::Instant,
};
/// Locally chosen input and bounded explicit spending, never decoded from an incoming offer.
pub struct Work {
    /// Complete application-owned input validated through installed requester codecs.
    pub input: Value,
    /// Explicit total bound and optional preagreed third-party payment.
    pub spending: Spending,
}
/// Trusted application policy decides whether to become a requester; a remote invitation cannot compel this callback to approve.
pub trait Policy: Send + Sync {
    /// Select local input/spending for this exact wanted contract and authenticated prospective provider.
    /// # Errors
    /// Refusal or unavailable input must produce no proposal, reservation or execution.
    fn choose(&self, selection: &Selection, provider: IdentityId) -> Result<Work, Error>;
}
/// Separate incoming observation, outgoing application authorization and current owner terms for handling an offer.
pub struct PublisherContexts<'a> {
    /// Trusted incoming authenticated offer authority.
    pub incoming: &'a Context<'a>,
    /// Explicit local requester/application permission.
    pub request: &'a Context<'a>,
    /// Independently issued Services Request permission.
    pub services: &'a services::contract::Context<'a>,
    /// Bounded authoritative current local wanted publication check.
    pub current: &'a dyn Current,
}
/// Convert a bounded invitation into a locally authorized proposal only after application policy chooses input and spending.
/// # Errors
/// Wrong publisher/roles/fields, stale terms, unsupported installed codecs or policy refusal produces no proposal or hold.
pub fn request(
    client: &Client,
    contexts: PublisherContexts<'_>,
    association: Association,
    selection: Selection,
    message: &Message,
    policy: &dyn Policy,
) -> Result<Request, Error> {
    tracing::trace!("Handling a wanted invitation through explicit local application policy");
    contexts.incoming.check(
        &client.authority,
        client.local,
        Action::Invoke,
        message.bytes().len(),
    )?;
    contexts
        .request
        .check(&client.authority, client.local, Action::Request, 0)?;
    contexts.services.check(
        &client.services,
        client.local,
        services::authority::Action::Request,
        0,
    )?;
    let o = selection.offering();
    if message.kind() != Kind::Offer
        || o.direction != Direction::Wanted
        || selection.publisher() != client.local
        || association.local != client.local
        || selection.roles().0 != client.local
        || selection.roles().1 != association.peer
        || association.operation.requester != client.local
    {
        return Err(Error::Denied);
    }
    let v = message.value();
    if v["offeringId"].as_str() != Some(o.id.as_str())
        || v["service"].as_str() != Some(o.service.as_str())
        || super::scalar::uint32(&v["serviceVersion"])? != o.version
    {
        return Err(Error::Conflict);
    }
    contexts.current.check(&selection)?;
    let work = policy.choose(&selection, association.peer)?;
    contexts.current.check(&selection)?;
    client.request(
        contexts.request,
        contexts.services,
        association,
        selection,
        work.input,
        work.spending,
    )
}
/// Original prospective-provider invitation, scoped to its owner and authenticated publisher stream.
pub struct Offered {
    association: Association,
    selection: Selection,
    bytes: Vec<u8>,
    deadline: Instant,
    domain: Weak<()>,
}
impl Offered {
    /// Borrow bounded offer JSON; it contains no fabricated application input or installation instructions.
    pub fn bytes(&self) -> &[u8] {
        tracing::trace!("Borrowing prospective-provider wanted invitation");
        &self.bytes
    }
    /// Read original trusted publisher/stream association.
    pub fn association(&self) -> Association {
        tracing::trace!("Reading wanted invitation association");
        self.association
    }
}
/// Explicitly installed prospective provider; remote definitions never install handlers or bypass local policy.
pub struct Offerer {
    local: IdentityId,
    authority: Arc<Authority>,
    services: Arc<services::authority::Authority>,
    registry: Arc<Registry>,
    limits: Limits,
    gate: Arc<dyn Gate>,
    domain: Arc<()>,
}
impl Offerer {
    /// Configure real local provider Registry and read-only policy under separate authority domains.
    /// # Errors
    /// Invalid bounds or a foreign registry fails before emitting an invitation.
    pub fn new(
        local: IdentityId,
        authority: Arc<Authority>,
        services: Arc<services::authority::Authority>,
        registry: Arc<Registry>,
        limits: Limits,
        gate: Arc<dyn Gate>,
    ) -> Result<Self, Error> {
        tracing::trace!("Creating an explicit wanted-offering prospective provider");
        limits.validate()?;
        if registry.local_identity() != local {
            return Err(Error::Denied);
        }
        Ok(Self {
            local,
            authority,
            services,
            registry,
            limits,
            gate,
            domain: Arc::new(()),
        })
    }
    /// Propose serving a retrieved wanted contract only when its exact local provider implementation exists.
    /// # Errors
    /// Wrong roles, unsupported/uninstalled providers, stale terms and caller cancellation cannot produce an offer.
    pub fn offer(
        &self,
        context: &Context<'_>,
        services_context: &services::contract::Context<'_>,
        association: Association,
        selection: Selection,
        current: &dyn Current,
    ) -> Result<Offered, Error> {
        tracing::trace!("Proposing locally supported work for an authenticated wanted publisher");
        context.check(&self.authority, self.local, Action::Request, 0)?;
        if association.local != self.local
            || selection.offering().direction != Direction::Wanted
            || selection.publisher() != association.peer
            || selection.roles().0 != association.peer
            || selection.roles().1 != self.local
            || association.operation.requester != association.peer
        {
            return Err(Error::Denied);
        }
        current.check(&selection)?;
        services::authorization::authorize(
            services_context,
            &self.services,
            self.local,
            &self.registry,
            selection.clone(),
            super::acceptance::peer(association),
            None,
        )?;
        let o = selection.offering();
        let bytes=serde_json::to_vec(&json!({"type":"offer","offeringId":o.id,"service":o.service,"serviceVersion":o.version})).map_err(|_|Error::Invalid)?;
        Message::parse(&bytes, self.limits.nodes)?;
        context.check(&self.authority, self.local, Action::Request, bytes.len())?;
        Ok(Offered {
            association,
            selection,
            bytes,
            deadline: context.deadline.min(services_context.deadline),
            domain: Arc::downgrade(&self.domain),
        })
    }
    /// Validate the publisher's proposal only as a reply on this original wanted invitation stream.
    /// # Errors
    /// Foreign/stale invitations, wrong stream/roles or invalid proposal metadata cannot authorize work.
    pub fn proposal(
        &self,
        context: &Context<'_>,
        services_context: &services::contract::Context<'_>,
        offered: Offered,
        observed: Association,
        message: &Message,
    ) -> Result<Proposal, Error> {
        tracing::trace!("Validating a wanted publisher's locally authorized proposal");
        if !Arc::ptr_eq(
            &offered.domain.upgrade().ok_or(Error::Denied)?,
            &self.domain,
        ) || observed != offered.association
        {
            return Err(Error::Denied);
        }
        if Instant::now() >= offered.deadline {
            return Err(Error::Cancelled);
        }
        let authorizer = Authorizer::new(
            self.local,
            self.authority.clone(),
            self.services.clone(),
            self.registry.clone(),
            self.limits,
            self.gate.clone(),
        )?;
        authorizer.authorize(
            context,
            services_context,
            observed,
            offered.selection,
            message,
        )
    }
}
