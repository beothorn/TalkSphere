//! Original-stream metadata requests and direct publisher attribution through public Board cache contracts.
use super::{
    Error,
    authority::{Action, Authority},
    contract::{Association, Context},
    message::{Kind, Message},
};
use crate::modules::board::{
    self,
    cache::{Cache, Provenance, Remote},
};
use serde_json::{Value, json};
use std::{sync::Arc, time::Instant};
enum RequestKind {
    Board,
    Definition { offering: String, receipt: Remote },
}
/// One authorized local request, bound to its original authenticated stream and optional selected receipt.
pub struct Request {
    authority: Arc<Authority>,
    association: Association,
    deadline: Instant,
    kind: RequestKind,
}
impl Request {
    /// Authorize a Board query without opening a socket or storing unauthenticated data.
    /// # Errors
    /// Rejects foreign/wrong-action local authority and invalid stream/requester association.
    pub fn board(
        context: &Context<'_>,
        authority: Arc<Authority>,
        association: Association,
    ) -> Result<Self, Error> {
        tracing::trace!("Authorizing one original-stream Board query");
        validate(context, &authority, association, 0)?;
        Ok(Self {
            authority,
            association,
            deadline: context.deadline,
            kind: RequestKind::Board,
        })
    }
    /// Authorize an exact definition query against a directly authenticated selected Board receipt.
    /// # Errors
    /// Forwarded/wrong-publisher receipts, empty IDs and insufficient authority fail without a request.
    pub fn definition(
        context: &Context<'_>,
        authority: Arc<Authority>,
        association: Association,
        offering: &str,
        receipt: Remote,
    ) -> Result<Self, Error> {
        tracing::trace!("Authorizing an exact current definition query");
        validate(context, &authority, association, offering.len())?;
        if offering.len() > super::record::CONTROL_MAX {
            return Err(Error::Limit);
        }
        if offering.is_empty() || receipt.provenance() != Provenance::Direct(association.peer) {
            return Err(Error::Denied);
        }
        Ok(Self {
            authority,
            association,
            deadline: context.deadline,
            kind: RequestKind::Definition {
                offering: offering.into(),
                receipt,
            },
        })
    }
    /// Return the authorized metadata request control fields; actual sending remains the exchange owner's responsibility.
    pub fn wire(&self) -> Value {
        tracing::trace!("Constructing authorized metadata request fields");
        match &self.kind {
            RequestKind::Board => json!({"type":"board.get"}),
            RequestKind::Definition { offering, .. } => {
                json!({"type":"service.get","offeringId":offering})
            }
        }
    }
    /// Consume a complete normally ended metadata reply on the original authenticated association.
    /// Incoming authority is separate from local Request authority; Cache supplies its own provenance grant.
    /// # Errors
    /// Wrong peers/streams/kinds, late/revoked callbacks, malformed documents and stale receipts fail before cache replacement.
    pub fn apply(
        self,
        context: &Context<'_>,
        observed: Association,
        response: &Message,
        cache: &mut Cache,
        cache_context: &board::contract::Context<'_>,
        now: u64,
    ) -> Result<Remote, Error> {
        tracing::trace!("Installing complete metadata with authenticated publisher provenance");
        context.check(
            &self.authority,
            self.association.local,
            Action::Invoke,
            response.bytes().len(),
        )?;
        if observed != self.association {
            return Err(Error::Denied);
        }
        if Instant::now() >= self.deadline {
            return Err(Error::Cancelled);
        }
        match self.kind {
            RequestKind::Board => {
                if response.kind() != Kind::Board {
                    return Err(Error::Invalid);
                }
                let bytes = response.field_bytes("board")?.ok_or(Error::Invalid)?;
                cache
                    .store(cache_context, Provenance::Direct(observed.peer), bytes, now)
                    .map_err(Error::from)
            }
            RequestKind::Definition { offering, receipt } => {
                if response.kind() != Kind::Definition {
                    return Err(Error::Invalid);
                }
                let bytes = response.field_bytes("definition")?.ok_or(Error::Invalid)?;
                cache
                    .attach(cache_context, &receipt, &offering, bytes, now)
                    .map_err(Error::from)
            }
        }
    }
}
// Local input authority cannot assert peer authentication; incoming callbacks must present their own Invoke grant.
fn validate(
    context: &Context<'_>,
    authority: &Authority,
    association: Association,
    bytes: usize,
) -> Result<(), Error> {
    tracing::trace!("Checking metadata query authority and original stream identity");
    context.check(authority, association.local, Action::Request, bytes)?;
    if association.operation.stream == 0 || association.operation.requester != association.local {
        return Err(Error::Invalid);
    }
    Ok(())
}
