//! Bounded transient fee negotiation bound to authenticated control stream context.
use super::{
    Error,
    authority::{Action, Authority},
    contract::Context,
    transfer::Order,
    transfer_crypto,
};
use crate::modules::identity::api::IdentityId;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
/// Trusted transport context supplied by an authenticated adapter, never decoded from payload fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Peer {
    /// Authenticated owner for this control exchange.
    pub owner: IdentityId,
    /// Unique local connection instance.
    pub connection: [u8; 32],
    /// Stream instance on which exact fee acceptance must arrive.
    pub stream: u32,
}
/// Fully validated order and explicit local fee policy result.
pub struct Request {
    /// Canonical unsigned order.
    pub order: Order,
    /// Owner key and signature over exact unsigned bytes.
    pub key: [u8; 32],
    /// Exact Ed25519 signature.
    pub signature: [u8; 64],
    /// Owner's explicit fee ceiling.
    pub maximum_fee: u64,
    /// Local issuer policy quote; a caller must not substitute peer input as this value.
    pub fee: u64,
    /// Finite monotonic expiry in the adapter's clock units.
    pub expires: u64,
}
/// Opaque live quote; cannot be converted to commitment without same-stream acceptance.
#[derive(Clone)]
pub struct Quote {
    peer: Peer,
    order: Order,
    fee: u64,
    expires: u64,
}
impl Quote {
    /// Exact fee to report to the owner; quote creation has no balance effect.
    pub fn fee(&self) -> u64 {
        tracing::trace!("Reading exact transfer fee quote");
        self.fee
    }
}
/// Accepted transfer capability; only the issuing quote owner constructs it.
pub struct Accepted {
    pub(super) authority: Arc<Authority>,
    pub(super) order: Order,
    pub(super) fee: u64,
}
/// Transient bounded live-stream quotes. Restart/disconnect discards them, never durable transfers.
pub struct Quotes {
    local: IdentityId,
    authority: Arc<Authority>,
    maximum: usize,
    live: Mutex<BTreeMap<Peer, Quote>>,
}
impl Quotes {
    /// Create a finite quote registry; no accounting or network effects.
    /// # Errors
    /// Zero capacity is invalid.
    pub fn new(
        local: IdentityId,
        authority: Arc<Authority>,
        maximum: usize,
    ) -> Result<Self, Error> {
        tracing::trace!("Creating bounded transfer fee registry");
        if maximum == 0 {
            return Err(Error::Invalid);
        }
        Ok(Self {
            local,
            authority,
            maximum,
            live: Mutex::new(BTreeMap::new()),
        })
    }
    /// Validate a signed authenticated owner request before retaining a finite exact quote.
    /// # Errors
    /// Denied proof/context, fee/bound/expiry failure or quote capacity rejects without charge.
    pub fn quote(
        &self,
        context: &Context<'_>,
        peer: Peer,
        request: Request,
        now: u64,
    ) -> Result<Quote, Error> {
        tracing::trace!("Quoting exact transfer fee on authenticated stream");
        context.check(&self.authority, self.local, Action::Control, 121)?;
        transfer_crypto::verify(
            peer.owner,
            self.local,
            &request.order,
            &request.key,
            &request.signature,
        )?;
        if request.fee >= request.order.amount
            || request.fee > request.maximum_fee
            || request.expires <= now
        {
            return Err(Error::Invalid);
        }
        let mut live = self.live.lock().map_err(|_| Error::Unknown)?;
        live.retain(|_, q| q.expires > now);
        if live.len() >= self.maximum && !live.contains_key(&peer) {
            return Err(Error::Limit);
        }
        if live.contains_key(&peer) {
            return Err(Error::Conflict);
        }
        let q = Quote {
            peer,
            order: request.order,
            fee: request.fee,
            expires: request.expires,
        };
        live.insert(peer, q.clone());
        Ok(q)
    }
    /// Consume one same-stream exact fee acceptance; accounting must recheck funds separately.
    /// # Errors
    /// Wrong peer, changed fee, expired/stale quote or authority fails without commitment.
    pub fn accept(
        &self,
        context: &Context<'_>,
        quote: &Quote,
        peer: Peer,
        fee: u64,
        now: u64,
    ) -> Result<Accepted, Error> {
        tracing::trace!("Accepting exact same-stream transfer fee");
        context.check(&self.authority, self.local, Action::Control, 121)?;
        if quote.peer != peer || quote.fee != fee || now >= quote.expires {
            return Err(Error::Denied);
        }
        let mut live = self.live.lock().map_err(|_| Error::Unknown)?;
        let current = live.get(&peer).ok_or(Error::Denied)?;
        if current.order != quote.order || current.fee != fee || current.expires != quote.expires {
            return Err(Error::Denied);
        }
        live.remove(&peer);
        Ok(Accepted {
            authority: self.authority.clone(),
            order: quote.order.clone(),
            fee,
        })
    }
    /// Release one disconnected exchange's transient quote without changing durable replay state.
    /// # Errors
    /// Invalid local authority or poisoned registry rejects cleanup.
    pub fn discard(&self, context: &Context<'_>, peer: Peer) -> Result<(), Error> {
        tracing::trace!("Discarding unaccepted transfer quote");
        context.check(&self.authority, self.local, Action::Control, 0)?;
        self.live.lock().map_err(|_| Error::Unknown)?.remove(&peer);
        Ok(())
    }
}
impl Accepted {
    /// Check the intended local authority without exposing the privileged issuer itself.
    /// # Errors
    /// Denies foreign authority domains or the wrong local issuer identity.
    pub fn check_receiver(
        &self,
        authority: &Arc<Authority>,
        local: IdentityId,
    ) -> Result<(), Error> {
        tracing::trace!("Checking fee acceptance receiver association");
        if !Arc::ptr_eq(&self.authority, authority) || self.order.issuer != local {
            return Err(Error::Denied);
        }
        Ok(())
    }
    /// Borrow the original exact public order after fee acceptance.
    pub fn order(&self) -> &Order {
        tracing::trace!("Reading accepted transfer order");
        &self.order
    }
    /// Read the original accepted exact fee, without granting accounting authority.
    pub fn fee(&self) -> u64 {
        tracing::trace!("Reading accepted transfer fee");
        self.fee
    }
}

#[cfg(test)]
#[path = "tests/fees.rs"]
mod tests;
