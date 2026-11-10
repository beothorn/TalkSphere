//! Exact service acceptance creates one durable local reservation before dispatch or wire acceptance.
use super::{
    Error,
    authority::Action,
    base64,
    contract::{Association, Context},
    message::{Kind, Message},
    quote::{Issued, Issuer},
    requester::Consented,
    value::Pending,
};
use crate::modules::{
    board::selection::Selection,
    services::{
        self,
        handler::{Invocation, Peer},
        persistence::Persistence,
        pricing::Price,
        quote::{Current, Issue, Quoter},
        runtime::Coordinator,
        value::Value,
    },
};
use serde_json::json;
use std::time::Instant;

/// Reserved provider attempt, still requiring complete input and both input terminal signals before dispatch.
pub struct Incoming {
    pub(crate) association: Association,
    pub(crate) input: Pending,
    pub(crate) selection: Selection,
    pub(crate) price: Price,
}
impl Incoming {
    /// Read original operation binding for durable observation/recovery without granting execution.
    pub fn association(&self) -> Association {
        tracing::trace!("Reading reserved provider association");
        self.association
    }
    /// Borrow accepted immutable contract and pricing terms.
    pub fn selection(&self) -> &Selection {
        tracing::trace!("Borrowing reserved provider terms");
        &self.selection
    }
    /// Read frozen accepted service price authorization.
    pub fn price(&self) -> Price {
        tracing::trace!("Reading reserved provider price");
        self.price
    }
    /// Inspect unresolved input declarations; these are not complete service input.
    pub fn input(&self) -> &Pending {
        tracing::trace!("Borrowing reserved input declarations before terminal completion");
        &self.input
    }
}
/// Requester attempt durably reserved and marked dispatched before its first accept byte may be sent.
pub struct Outgoing {
    pub(crate) association: Association,
    pub(crate) invocation: Invocation,
    pub(crate) input: Value,
    accept: Vec<u8>,
}
impl Outgoing {
    /// Borrow exact accept JSON only after local durable reservation and once-only dispatch marking.
    pub fn accept(&self) -> &[u8] {
        tracing::trace!("Borrowing accept after durable requester reservation");
        &self.accept
    }
    /// Read original authenticated operation binding retained through lost sends.
    pub fn association(&self) -> Association {
        tracing::trace!("Reading reserved requester association");
        self.association
    }
    /// Borrow immutable original accepted invocation; remote loss does not remove its journal.
    pub fn invocation(&self) -> &Invocation {
        tracing::trace!("Borrowing retained requester invocation");
        &self.invocation
    }
    /// Borrow complete locally authorized input for subsequent bounded body transmission.
    pub fn input(&self) -> &Value {
        tracing::trace!("Borrowing reserved original requester input");
        &self.input
    }
}
impl Issuer {
    /// Consume one original provider quote after exact ID/hash, stream and current-term validation, then reserve durably.
    /// # Errors
    /// Malformed/foreign/stale acceptance cannot reserve. Admission/storage uncertainty retains original-key state and never implies rollback.
    pub fn accept<P: Persistence>(
        &self,
        context: &Context<'_>,
        dependencies: Admission<'_, P>,
        issued: Issued,
        observed: Association,
        message: &Message,
    ) -> Result<Incoming, Error> {
        tracing::trace!("Reserving provider capacity after exact original quote acceptance");
        context.check(
            &self.authority,
            self.local,
            Action::Invoke,
            message.bytes().len(),
        )?;
        self.owns(&issued)?;
        // A rejected exchange releases only its original transient quote, never any admitted durable operation.
        let checked = (|| -> Result<Selection, Error> {
            if observed != issued.association || message.kind() != Kind::Accept {
                return Err(Error::Denied);
            }
            if Instant::now() >= issued.expires {
                return Err(Error::Cancelled);
            }
            let v = message.value();
            let id: [u8; 32] = base64::decode(v["quoteId"].as_str().ok_or(Error::Invalid)?, 32)?
                .try_into()
                .map_err(|_| Error::Invalid)?;
            let hash: [u8; 32] =
                base64::decode(v["quoteHash"].as_str().ok_or(Error::Invalid)?, 32)?
                    .try_into()
                    .map_err(|_| Error::Invalid)?;
            if id != issued.id() || hash != issued.hash() {
                return Err(Error::Denied);
            }
            let accepted = self.quoter.accept(
                dependencies.quote_context,
                &issued.service,
                peer(observed),
                hash,
                dependencies.current,
            )?;
            if accepted.price() != issued.price {
                return Err(Error::Conflict);
            }
            let selection = accepted.assessed().authorized().selection().clone();
            let key = dependencies.runtime.admit(
                dependencies.coordinate_context,
                accepted,
                issued.payment,
            )?;
            if key != peer(observed).key {
                return Err(Error::Unknown);
            }
            Ok(selection)
        })();
        let selection = match checked {
            Ok(selection) => selection,
            Err(error) => {
                self.discard(context, dependencies.quote_context, &issued)?;
                return Err(error);
            }
        };
        Ok(Incoming {
            association: observed,
            input: issued.input,
            selection,
            price: issued.price,
        })
    }
}
/// Distinct Services quote/admission grants plus bounded current-term validation and the real durable coordinator.
pub struct Admission<'a, P: Persistence> {
    /// Invoke on provider, Request on requester, scoped to the exact local Services domain.
    pub quote_context: &'a services::contract::Context<'a>,
    /// Separate trusted local Coordinate grant; no decoded peer message contains this authority.
    pub coordinate_context: &'a services::contract::Context<'a>,
    /// Authoritative current-term checker appropriate to this endpoint.
    pub current: &'a dyn Current,
    /// Local durable operation owner, independent of the Transport session.
    pub runtime: &'a Coordinator<P>,
}
/// Reserve one explicitly consented requester attempt before returning any accept bytes.
/// # Errors
/// Wrong authority/roles, expiry, changed terms/pricing, exhausted funds and durable uncertainty fail without granting send permission.
pub(crate) fn reserve<P: Persistence>(
    client: &super::requester::Client,
    context: &Context<'_>,
    dependencies: Admission<'_, P>,
    consent: Consented,
) -> Result<Outgoing, Error> {
    tracing::trace!("Durably reserving requester credits before wire acceptance");
    let candidate = consent.candidate;
    context.check(
        &client.authority,
        client.local,
        Action::Request,
        candidate.bytes.bytes().len(),
    )?;
    if candidate.association.local != client.local
        || candidate.association.operation.requester != client.local
    {
        return Err(Error::Denied);
    }
    if Instant::now() >= candidate.deadline {
        return Err(Error::Cancelled);
    }
    let quoter = Quoter::new(client.local, client.services.clone(), client.service_limits)?;
    let original_input = candidate
        .assessed
        .authorized()
        .input()
        .ok_or(Error::Denied)?
        .clone();
    let quote = quoter.issue(
        dependencies.quote_context,
        Issue {
            assessed: candidate.assessed,
            bytes: candidate.bytes,
            maximum: candidate.maximum,
            expires: candidate.deadline.min(context.deadline),
        },
    )?;
    if quote.price() != candidate.price || quote.hash() != candidate.hash {
        return Err(Error::Conflict);
    }
    let accepted = quoter.accept(
        dependencies.quote_context,
        &quote,
        peer(candidate.association),
        candidate.hash,
        dependencies.current,
    )?;
    let key =
        dependencies
            .runtime
            .admit(dependencies.coordinate_context, accepted, candidate.payment)?;
    // Mark the original attempt before exposing accept bytes; repeated admissions cannot dispatch it twice.
    dependencies
        .runtime
        .supply_input(dependencies.coordinate_context, key, original_input)?;
    let (invocation, input) = dependencies
        .runtime
        .outbound(dependencies.coordinate_context, key)?;
    let accept=serde_json::to_vec(&json!({"type":"accept","quoteId":base64::encode(&candidate.id),"quoteHash":base64::encode(&candidate.hash)})).map_err(|_|Error::Invalid)?;
    Ok(Outgoing {
        association: candidate.association,
        invocation,
        input,
        accept,
    })
}
// Preserve exactly the original operation key across Peer Protocol and Services authorities.
pub(crate) fn peer(a: Association) -> Peer {
    tracing::trace!("Mapping accepted original stream to Services operation");
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
