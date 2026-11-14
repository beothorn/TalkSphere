//! Requester read controls preserve authenticated original-issuer provenance and wait for complete normal exchange termination.
use crate::modules::{
    identity::{api::IdentityId, text},
    ledger::{transfer::Order, transfer_state::TransferResult},
    peer_protocol::{
        Error,
        authority::{Action, Authority},
        contract::{Association, Context},
        failure::Failure,
        message::{Kind, Message},
        scalar,
    },
};
use serde_json::json;
use std::{sync::Arc, time::Instant};
enum Query {
    Balance,
    Transfer { owner: IdentityId, id: [u8; 16] },
}
/// Original outgoing query; forwarded JSON cannot replace its authenticated peer/connection/stream binding.
pub struct Request {
    authority: Arc<Authority>,
    association: Association,
    deadline: Instant,
    query: Query,
    bytes: Vec<u8>,
}
/// Completed independent query observation, without any implicit local balance overwrite or payment effect.
pub enum Response {
    /// Original issuer's exact caller-scoped balance.
    Balance(u64),
    /// Original issuer's complete protected transfer response.
    Transfer(Box<Transfer>),
}
/// Opaque complete transfer observation; its issuer is the authenticated responder, never a requester-supplied JSON field.
pub struct Transfer {
    association: Association,
    result: TransferResult,
}
impl Transfer {
    // Only sibling authenticated Ledger control paths can mint a normally completed issuer observation.
    pub(super) fn observed(association: Association, result: TransferResult) -> Self {
        tracing::trace!("Retaining a normally completed issuer transfer observation");
        Self {
            association,
            result,
        }
    }

    /// Read trusted original issuer identity from the independent authenticated exchange.
    pub fn issuer(&self) -> IdentityId {
        tracing::trace!("Reading authenticated transfer-query issuer");
        self.association.peer
    }
    /// Read original query association for settlement correlation and cross-stream isolation.
    pub fn association(&self) -> Association {
        tracing::trace!("Reading authenticated transfer query association");
        self.association
    }
    /// Borrow exact original gross, fee, net and identities after normal response completion.
    pub fn result(&self) -> &TransferResult {
        tracing::trace!("Borrowing complete original-issuer transfer observation");
        &self.result
    }
}
impl Request {
    /// Create one caller-scoped balance request to an independently authenticated issuer.
    /// # Errors
    /// Wrong authority/roles, cancellation or invalid original stream is denied before wire output.
    pub fn balance(
        context: &Context<'_>,
        authority: Arc<Authority>,
        association: Association,
    ) -> Result<Self, Error> {
        tracing::trace!("Creating an explicit issuer balance query");
        Self::new(context, authority, association, Query::Balance)
    }
    /// Query one exact owner/transfer tuple without granting access to unrelated issuer records.
    /// # Errors
    /// Wrong local purpose/association or exhausted caller grants fail without effects.
    pub fn transfer(
        context: &Context<'_>,
        authority: Arc<Authority>,
        association: Association,
        owner: IdentityId,
        id: [u8; 16],
    ) -> Result<Self, Error> {
        tracing::trace!("Creating an independent issuer transfer-result query");
        Self::new(
            context,
            authority,
            association,
            Query::Transfer { owner, id },
        )
    }
    // Preserve the original route/deadline and exact outgoing bytes before any response can be applied.
    fn new(
        context: &Context<'_>,
        authority: Arc<Authority>,
        association: Association,
        query: Query,
    ) -> Result<Self, Error> {
        tracing::trace!("Binding a finite original authenticated Ledger query");
        if association.operation.requester != association.local
            || association.operation.stream == 0
            || association.local == association.peer
        {
            return Err(Error::Denied);
        }
        let value = match query {
            Query::Balance => json!({"type":"ledger.balance"}),
            Query::Transfer { owner, id } => {
                json!({"type":"ledger.transfer.get","ownerId":text::format(owner),"transferId":super::super::base64::encode(&id)})
            }
        };
        let bytes = serde_json::to_vec(&value).map_err(|_| Error::Invalid)?;
        context.check(&authority, association.local, Action::Request, bytes.len())?;
        Ok(Self {
            authority,
            association,
            deadline: context.deadline,
            query,
            bytes,
        })
    }
    /// Borrow the exact bounded outgoing query control.
    pub fn bytes(&self) -> &[u8] {
        tracing::trace!("Borrowing outgoing independent Ledger query bytes");
        &self.bytes
    }
    /// Publish one observation only after exact expected response and normal original-stream completion.
    /// # Errors
    /// Wrong attribution/fields, missing/abnormal termination, changed tuple, invalid arithmetic and explicit failure cannot become issuer evidence.
    pub fn finish(
        self,
        context: &Context<'_>,
        observed: Association,
        message: &Message,
        normal: bool,
    ) -> Result<Response, Error> {
        tracing::trace!("Completing an authenticated independent Ledger query");
        context.check(
            &self.authority,
            self.association.local,
            Action::Invoke,
            message.bytes().len(),
        )?;
        if observed != self.association {
            return Err(Error::Denied);
        }
        if Instant::now() >= self.deadline {
            return Err(Error::Cancelled);
        }
        if !normal {
            return Err(Error::Unknown);
        }
        if message.kind() == Kind::Failure {
            return Err(Failure::parse(message)?.code.error());
        }
        match self.query {
            Query::Balance if message.kind() == Kind::LedgerBalanceResult => Ok(Response::Balance(
                scalar::uint64(&message.value()["balance"])?,
            )),
            Query::Transfer { owner, id } if message.kind() == Kind::LedgerTransferResult => {
                let result = decode(self.association.peer, message)?;
                if result.order.owner != owner
                    || result.order.transfer_id != id
                    || (self.association.local != result.order.owner
                        && self.association.local != result.order.new_owner)
                {
                    return Err(Error::Denied);
                }
                Ok(Response::Transfer(Box::new(Transfer::observed(
                    observed, result,
                ))))
            }
            _ => Err(Error::Invalid),
        }
    }
}
// Canonical wire amounts and identities become a validated order only with independently authenticated issuer attribution.
pub(crate) fn decode(issuer: IdentityId, message: &Message) -> Result<TransferResult, Error> {
    tracing::trace!("Decoding exact authenticated Ledger transfer gross/fee/net");
    if message.kind() != Kind::LedgerTransferResult {
        return Err(Error::Invalid);
    }
    let v = message.value();
    let result = TransferResult {
        order: Order {
            owner: text::parse(v["ownerId"].as_str().ok_or(Error::Invalid)?)
                .map_err(|_| Error::Invalid)?,
            new_owner: text::parse(v["newOwnerId"].as_str().ok_or(Error::Invalid)?)
                .map_err(|_| Error::Invalid)?,
            issuer,
            transfer_id: super::super::base64::decode(
                v["transferId"].as_str().ok_or(Error::Invalid)?,
                16,
            )?
            .try_into()
            .map_err(|_| Error::Invalid)?,
            amount: scalar::uint64(&v["amount"])?,
        },
        fee: scalar::uint64(&v["fee"])?,
    };
    result.order.encode()?;
    if result.received()? != scalar::uint64(&v["receivedAmount"])? {
        return Err(Error::Invalid);
    }
    Ok(result)
}
