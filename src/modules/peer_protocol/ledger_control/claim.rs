//! Fixed-size encrypted claim controls use authenticated Ledger/Identity decryption ports and expose only public issuer results.
use crate::modules::{
    identity::api::IdentityId,
    ledger::{
        self,
        persistence::Persistence,
        redemption::{Decryptor, ENVELOPE_BYTES},
        state::Ledger,
    },
    peer_protocol::{
        Error,
        authority::{Action, Authority},
        contract::{Association, Context, Limits},
        failure::Failure,
        message::{Kind, Message},
        scalar,
    },
};
use std::{sync::Arc, time::Instant};
/// Separate real accounting/decryption dependencies; no peer supplies these grants or key handles.
pub struct Ports<'a, P: Persistence, D: Decryptor> {
    /// Original local issuer Ledger.
    pub owner: &'a Ledger<P>,
    /// Purpose-scoped local Ledger Control context.
    pub control: &'a ledger::contract::Context<'a>,
    /// Protected authenticated complete-decryption port, normally IdentityDecryptor.
    pub decryptor: &'a D,
}
/// Redeem one exact generic encrypted object for the authenticated nonlocal caller.
/// # Errors
/// Invalid/oversized ciphertext is rejected before decryption; wrong owner/secret, unavailable keys, storage or authority cannot create success.
pub fn respond<P: Persistence, D: Decryptor>(
    context: &Context<'_>,
    authority: &Authority,
    association: Association,
    message: &Message,
    ports: Ports<'_, P, D>,
    limits: Limits,
) -> Result<super::super::record::Record, Error> {
    tracing::trace!("Redeeming a bounded authenticated encrypted Ledger claim");
    context.check(
        authority,
        association.local,
        Action::Invoke,
        message.bytes().len(),
    )?;
    limits.validate()?;
    if association.local != ports.owner.local_identity()
        || association.peer == association.local
        || association.operation.requester != association.peer
        || association.operation.stream == 0
    {
        return Err(Error::Denied);
    }
    if message.kind() != Kind::LedgerClaimRedeem {
        return Err(Error::Unsupported);
    }
    // Check the fixed public reply's complete shape before decrypting or committing issuance.
    // The all-zero public ID and longest amount are sizing values, never redemption inputs.
    super::read::encode(
        &serde_json::json!({"type":"ledger.claim.result","claimId":super::super::base64::encode(&[0;32]),"amount":u64::MAX.to_string()}),
        limits,
    )?;
    let object = super::super::base64::decode(
        message.value()["envelope"].as_str().ok_or(Error::Invalid)?,
        ENVELOPE_BYTES,
    )?;
    if object.len() != ENVELOPE_BYTES {
        return Err(Error::Invalid);
    }
    let result =
        ports
            .owner
            .redeem_claim(ports.control, association.peer, &object, ports.decryptor)?;
    super::read::encode(
        &serde_json::json!({"type":"ledger.claim.result","claimId":super::super::base64::encode(&result.id),"amount":result.amount.to_string()}),
        limits,
    ).map_err(|_| Error::Unknown)
}
/// Original outgoing encrypted redemption, retaining public expected claim ID and issuer provenance only.
pub struct Request {
    authority: Arc<Authority>,
    association: Association,
    deadline: Instant,
    id: [u8; 32],
    bytes: Vec<u8>,
}
/// Complete public issuer claim observation, with no bearer secret or implicit local balance overwrite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Redeemed {
    /// Authenticated original issuer, never a forwarded JSON identity.
    pub issuer: IdentityId,
    /// Exact original public claim ID.
    pub id: [u8; 32],
    /// Stored authoritative positive issuer amount, not a portable hint.
    pub amount: u64,
}
impl Request {
    /// Encode an already authenticated issuer-encrypted redemption under explicit local Request permission.
    /// # Errors
    /// Wrong route/size/purpose or exhausted byte grants fail without sending clear secrets or initiating issuance.
    pub fn new(
        context: &Context<'_>,
        authority: Arc<Authority>,
        association: Association,
        object: &[u8],
        id: [u8; 32],
    ) -> Result<Self, Error> {
        tracing::trace!("Creating a bounded encrypted claim redemption control");
        if association.operation.requester != association.local
            || association.operation.stream == 0
            || association.peer == association.local
        {
            return Err(Error::Denied);
        }
        if object.len() != ENVELOPE_BYTES {
            return Err(Error::Invalid);
        }
        let bytes=serde_json::to_vec(&serde_json::json!({"type":"ledger.claim.redeem","envelope":super::super::base64::encode(object)})).map_err(|_|Error::Invalid)?;
        context.check(&authority, association.local, Action::Request, bytes.len())?;
        Ok(Self {
            authority,
            association,
            deadline: context.deadline,
            id,
            bytes,
        })
    }
    /// Borrow only ciphertext control bytes; expected amounts and bearer plaintext are absent.
    pub fn bytes(&self) -> &[u8] {
        tracing::trace!("Borrowing encrypted claim control without plaintext");
        &self.bytes
    }
    /// Accept a positive exact-ID result only after normal completion from the original authenticated issuer.
    /// # Errors
    /// Missing/abnormal completion, wrong issuer/stream/ID, invalid amount and explicit failure cannot publish successful redemption.
    pub fn finish(
        self,
        context: &Context<'_>,
        observed: Association,
        message: &Message,
        normal: bool,
    ) -> Result<Redeemed, Error> {
        tracing::trace!("Completing a normally ended original-issuer claim redemption");
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
        if message.kind() != Kind::LedgerClaimResult {
            return Err(Error::Invalid);
        }
        let v = message.value();
        let id: [u8; 32] =
            super::super::base64::decode(v["claimId"].as_str().ok_or(Error::Invalid)?, 32)?
                .try_into()
                .map_err(|_| Error::Invalid)?;
        if id != self.id {
            return Err(Error::Denied);
        }
        let amount = scalar::uint64(&v["amount"])?;
        if amount == 0 {
            return Err(Error::Invalid);
        }
        Ok(Redeemed {
            issuer: self.association.peer,
            id,
            amount,
        })
    }
}
