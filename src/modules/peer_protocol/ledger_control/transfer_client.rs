//! Explicit owner fee consent is separate from signed order creation, issuer commitment and original payment reconciliation.
use super::query::{self, Transfer};
use crate::modules::{
    ledger::{transfer::Order, transfer_crypto},
    peer_protocol::{
        Error,
        authority::{Action, Authority},
        contract::{Association, Context},
        failure::Failure,
        message::{Kind, Message},
        record::Record,
        scalar,
    },
};
use std::{
    sync::{Arc, Weak},
    time::Instant,
};
/// Canonical owner-signed order and explicit requested fee ceiling, never a signer or private key.
pub struct Signed {
    /// Original exact positive gross order.
    pub order: Order,
    /// Owner's exact32-byte Ed25519 public key.
    pub key: [u8; 32],
    /// Exact64-byte signature over the121 canonical unsigned order bytes.
    pub signature: [u8; 64],
    /// Nonnegative maximum for a newly negotiated fee.
    pub maximum_fee: u64,
}
enum Stage {
    Waiting,
    Quoted(u64),
    Accepted(u64),
}
/// One original issuer transfer request, with no local Ledger mutation or implicit fee consent.
pub struct Request {
    authority: Arc<Authority>,
    association: Association,
    signed: Signed,
    deadline: Instant,
    wire: Vec<u8>,
    domain: Arc<()>,
    stage: Stage,
}
/// Exact original-stream fee offer for a separate explicit local approval call.
pub struct Offer {
    domain: Weak<()>,
    id: [u8; 16],
    fee: u64,
}
impl Offer {
    /// Read fee before deciding whether to accept; receiving a quote alone never grants consent.
    pub fn fee(&self) -> u64 {
        tracing::trace!("Reading an offered exact issuer fee");
        self.fee
    }
    /// Read exact order replay identity bound to this fee.
    pub fn id(&self) -> [u8; 16] {
        tracing::trace!("Reading original offered transfer identity");
        self.id
    }
}
/// Normally completed issuer observation; historical direct replies do not assert fresh fee acceptance.
pub struct Completion {
    proof: Transfer,
    accepted_fee: Option<u64>,
}
impl Completion {
    /// Borrow complete original-issuer observation for separately authorized reconciliation.
    pub fn proof(&self) -> &Transfer {
        tracing::trace!("Borrowing complete issuer transfer commitment observation");
        &self.proof
    }
    /// Read this exchange's exact explicitly accepted fee, or None for a direct stored retry result.
    pub fn accepted_fee(&self) -> Option<u64> {
        tracing::trace!("Reading fresh versus historical transfer fee consent");
        self.accepted_fee
    }
    /// Transfer ownership of the complete issuer observation without creating another payment.
    pub fn into_proof(self) -> Transfer {
        tracing::trace!("Retaining complete issuer transfer observation for reconciliation");
        self.proof
    }
}
impl Request {
    /// Validate owner/channel/signature and encode one bounded original transfer request.
    /// # Errors
    /// Wrong owner/issuer, bad canonical proof or caller authority fails before output; private signing is outside this adapter.
    pub fn new(
        context: &Context<'_>,
        authority: Arc<Authority>,
        association: Association,
        signed: Signed,
    ) -> Result<Self, Error> {
        tracing::trace!("Creating a signed explicit issuer transfer request");
        if association.operation.requester != association.local
            || association.operation.stream == 0
            || signed.order.owner != association.local
            || signed.order.issuer != association.peer
        {
            return Err(Error::Denied);
        }
        transfer_crypto::verify(
            association.local,
            association.peer,
            &signed.order,
            &signed.key,
            &signed.signature,
        )?;
        let wire=serde_json::to_vec(&serde_json::json!({"type":"ledger.transfer","order":super::super::base64::encode(&signed.order.encode()?),"ownerPublicKey":super::super::base64::encode(&signed.key),"signature":super::super::base64::encode(&signed.signature),"maximumFee":signed.maximum_fee.to_string()})).map_err(|_|Error::Invalid)?;
        context.check(&authority, association.local, Action::Request, wire.len())?;
        Ok(Self {
            authority,
            association,
            signed,
            deadline: context.deadline,
            wire,
            domain: Arc::new(()),
            stage: Stage::Waiting,
        })
    }
    /// Borrow the original signed order control; no fee acceptance is implied.
    pub fn bytes(&self) -> &[u8] {
        tracing::trace!("Borrowing signed issuer transfer request bytes");
        &self.wire
    }
    /// Validate one exact issuer fee on the original stream without authorizing its commitment.
    /// # Errors
    /// Duplicate/wrong-stage responses, foreign attribution, changed ID or fee above the maximum/amount are refused.
    pub fn quote(
        &mut self,
        context: &Context<'_>,
        observed: Association,
        message: &Message,
    ) -> Result<Offer, Error> {
        tracing::trace!("Validating original issuer fee before local acceptance");
        self.check(context, observed, Action::Invoke, message.bytes().len())?;
        if !matches!(self.stage, Stage::Waiting) || message.kind() != Kind::LedgerTransferQuote {
            return Err(Error::Invalid);
        }
        let v = message.value();
        let id: [u8; 16] =
            super::super::base64::decode(v["transferId"].as_str().ok_or(Error::Invalid)?, 16)?
                .try_into()
                .map_err(|_| Error::Invalid)?;
        let fee = scalar::uint64(&v["fee"])?;
        if id != self.signed.order.transfer_id {
            return Err(Error::Denied);
        }
        if fee > self.signed.maximum_fee || fee >= self.signed.order.amount {
            return Err(Error::Insufficient);
        }
        self.stage = Stage::Quoted(fee);
        Ok(Offer {
            domain: Arc::downgrade(&self.domain),
            id,
            fee,
        })
    }
    /// Explicitly accept only the original validated fee using local Request permission.
    /// # Errors
    /// Foreign/repeated offers, cancellation and wrong purpose cannot produce acceptance bytes.
    pub fn accept(&mut self, context: &Context<'_>, offer: &Offer) -> Result<Record, Error> {
        tracing::trace!("Recording explicit local consent to exact issuer transfer fee");
        self.check(context, self.association, Action::Request, 0)?;
        if !Arc::ptr_eq(&offer.domain.upgrade().ok_or(Error::Denied)?, &self.domain)
            || !matches!(self.stage,Stage::Quoted(fee) if fee==offer.fee)
        {
            return Err(Error::Denied);
        }
        let bytes=serde_json::to_vec(&serde_json::json!({"type":"ledger.transfer.accept","transferId":super::super::base64::encode(&offer.id),"fee":offer.fee.to_string()})).map_err(|_|Error::Invalid)?;
        context.check(
            &self.authority,
            self.association.local,
            Action::Request,
            bytes.len(),
        )?;
        self.stage = Stage::Accepted(offer.fee);
        Ok(Record::Control(bytes))
    }
    /// Observe exact signed-order commitment only after normal completion, preserving accepted versus stored-result distinction.
    /// # Errors
    /// Wrong stream/order/fee, unaccepted quoted transfers, explicit failures and abnormal termination cannot authorize reconciliation.
    pub fn finish(
        self,
        context: &Context<'_>,
        observed: Association,
        message: &Message,
        normal: bool,
    ) -> Result<Completion, Error> {
        tracing::trace!("Completing an authenticated issuer transfer result");
        self.check(context, observed, Action::Invoke, message.bytes().len())?;
        if !normal {
            return Err(Error::Unknown);
        }
        if message.kind() == Kind::Failure {
            return Err(Failure::parse(message)?.code.error());
        }
        let result = query::decode(self.association.peer, message)?;
        if result.order != self.signed.order {
            return Err(Error::Denied);
        }
        let accepted_fee = match self.stage {
            Stage::Waiting => None,
            Stage::Accepted(fee) if fee == result.fee => Some(fee),
            _ => return Err(Error::Invalid),
        };
        Ok(Completion {
            proof: Transfer::observed(observed, result),
            accepted_fee,
        })
    }
    // Local lifetime and original authenticated issuer route apply before every transition or observable output.
    fn check(
        &self,
        context: &Context<'_>,
        observed: Association,
        action: Action,
        bytes: usize,
    ) -> Result<(), Error> {
        context.check(&self.authority, self.association.local, action, bytes)?;
        if observed != self.association {
            return Err(Error::Denied);
        }
        if Instant::now() >= self.deadline {
            return Err(Error::Cancelled);
        }
        Ok(())
    }
}
