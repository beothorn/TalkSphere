//! Issuer transfer negotiation verifies signed retries before new fees and commits only after exact original-stream acceptance.
use super::read;
use crate::modules::{
    ledger::{self, fees, persistence::Persistence, state::Ledger, transfer::Order},
    peer_protocol::{
        Error,
        authority::{Action, Authority},
        contract::{Association, Context, Limits},
        message::{Kind, Message},
        record::Record,
        scalar,
    },
};
use std::{
    sync::{Arc, Weak},
    time::Instant,
};
/// Trusted local issuer fee policy; values decoded from peer input never implement this contract.
pub trait Policy: Send + Sync {
    /// Quote an exact nonnegative fee for the validated order without committing it.
    fn fee(&self, order: &Order) -> Result<u64, Error>;
}
/// Explicit fixed issuer policy; zero fees are allowed while positive net transfer remains required.
pub struct Fixed(pub u64);
impl Policy for Fixed {
    // Configuration supplies the fee rather than untrusted order metadata or maximumFee.
    fn fee(&self, _: &Order) -> Result<u64, Error> {
        tracing::trace!("Reading configured exact transfer fee");
        Ok(self.0)
    }
}
/// Trusted local monotonic observations; no wire timestamp controls quote lifetime.
pub struct Time {
    /// Current counter in milliseconds within this quote owner's lifetime.
    pub now_milliseconds: u64,
    /// Finite future counter when the unaccepted quote expires.
    pub expires_milliseconds: u64,
}
/// Explicit dependencies for one local issuer's bounded control handler.
pub struct Config<P: Persistence> {
    /// Incoming authenticated Peer Protocol grant issuer.
    pub peer: Arc<Authority>,
    /// Original Ledger authority used for Control contexts and fee capabilities.
    pub authority: Arc<ledger::authority::Authority>,
    /// Real local issuer Ledger owner.
    pub ledger: Arc<Ledger<P>>,
    /// Finite control and live-quote bounds.
    pub limits: Limits,
    /// Explicit trusted fee policy.
    pub policy: Arc<dyn Policy>,
}
/// Bounded original issuer transfer adapter; it never grants credits or performs Services work.
pub struct Issuer<P: Persistence> {
    config: Config<P>,
    quotes: fees::Quotes,
    domain: Arc<()>,
}
/// Unaccepted original fee quote; not a reservation guarantee or transfer result.
pub struct Quoted {
    quote: fees::Quote,
    association: Association,
    id: [u8; 16],
    wire: Record,
    deadline: Instant,
    domain: Weak<()>,
}
impl Quoted {
    /// Borrow exact transfer ID/fee control for bounded transmission.
    pub fn record(&self) -> &Record {
        tracing::trace!("Borrowing original issuer fee quote control");
        &self.wire
    }
    /// Read original authenticated owner/connection/stream.
    pub fn association(&self) -> Association {
        tracing::trace!("Reading original transfer quote association");
        self.association
    }
}
/// A signed accepted retry returns Stored directly; a new order requires exact fee acceptance.
pub enum Begin {
    /// Original already committed transfer, with no requote or new effect.
    Stored(Record),
    /// New bounded unaccepted fee quote.
    Quote(Box<Quoted>),
}
impl<P: Persistence> Issuer<P> {
    /// Configure finite fee ownership using actual Ledger and local policy.
    /// # Errors
    /// Invalid limits fail before live fee quotes exist.
    pub fn new(config: Config<P>) -> Result<Self, Error> {
        tracing::trace!("Creating an original issuer transfer control adapter");
        config.limits.validate()?;
        let quotes = fees::Quotes::new(
            config.ledger.local_identity(),
            config.authority.clone(),
            config.limits.quotes,
        )?;
        Ok(Self {
            config,
            quotes,
            domain: Arc::new(()),
        })
    }
    /// Recognize a signed accepted retry first; otherwise quote exact local fees without reserving money.
    /// # Errors
    /// Bad proof/roles/fields, capacity, expiry and storage fail; Insufficient here means fee exceeds the requested bound and maps to price_rejected.
    pub fn begin(
        &self,
        context: &Context<'_>,
        ledger_context: &ledger::contract::Context<'_>,
        association: Association,
        message: &Message,
        time: Time,
    ) -> Result<Begin, Error> {
        tracing::trace!("Recognizing original transfer or issuing a bounded exact fee quote");
        self.check(context, association, message.bytes().len())?;
        if message.kind() != Kind::LedgerTransfer {
            return Err(Error::Unsupported);
        }
        let v = message.value();
        let order = Order::decode(&super::super::base64::decode(
            v["order"].as_str().ok_or(Error::Invalid)?,
            121,
        )?)?;
        let key: [u8; 32] =
            super::super::base64::decode(v["ownerPublicKey"].as_str().ok_or(Error::Invalid)?, 32)?
                .try_into()
                .map_err(|_| Error::Invalid)?;
        let signature: [u8; 64] =
            super::super::base64::decode(v["signature"].as_str().ok_or(Error::Invalid)?, 64)?
                .try_into()
                .map_err(|_| Error::Invalid)?;
        if let Some(result) = self.config.ledger.existing_transfer(
            ledger_context,
            association.peer,
            &order,
            &key,
            &signature,
        )? {
            return Ok(Begin::Stored(read::transfer_result(
                &result,
                self.config.limits,
            )?));
        }
        let maximum = scalar::uint64(&v["maximumFee"])?;
        let fee = self.config.policy.fee(&order)?;
        if fee > maximum || fee >= order.amount {
            return Err(Error::Insufficient);
        }
        let deadline = context.deadline.min(ledger_context.deadline);
        let remaining = u64::try_from(
            deadline
                .saturating_duration_since(Instant::now())
                .as_millis(),
        )
        .map_err(|_| Error::Limit)?;
        let expires = time.expires_milliseconds.min(
            time.now_milliseconds
                .checked_add(remaining)
                .ok_or(Error::Limit)?,
        );
        if expires <= time.now_milliseconds {
            return Err(Error::Cancelled);
        }
        let id = order.transfer_id;
        let quote = self.quotes.quote(
            ledger_context,
            peer(association),
            fees::Request {
                order,
                key,
                signature,
                maximum_fee: maximum,
                fee,
                expires,
            },
            time.now_milliseconds,
        )?;
        let wire = read::encode(
            &serde_json::json!({"type":"ledger.transfer.quote","transferId":super::super::base64::encode(&id),"fee":fee.to_string()}),
            self.config.limits,
        )?;
        Ok(Begin::Quote(Box::new(Quoted {
            quote,
            association,
            id,
            wire,
            deadline: context.deadline.min(ledger_context.deadline),
            domain: Arc::downgrade(&self.domain),
        })))
    }
    /// Commit one exact original fee acceptance through Ledger's atomic balance/replay checks.
    /// # Errors
    /// Wrong owner/stream/ID/fee, stale quote, insufficient current funds and durable uncertainty cannot produce new successful output.
    pub fn accept(
        &self,
        context: &Context<'_>,
        ledger_context: &ledger::contract::Context<'_>,
        quote: &Quoted,
        observed: Association,
        message: &Message,
        now_milliseconds: u64,
    ) -> Result<Record, Error> {
        tracing::trace!("Committing a transfer only after exact fee acceptance");
        self.check(context, observed, message.bytes().len())?;
        if !Arc::ptr_eq(&quote.domain.upgrade().ok_or(Error::Denied)?, &self.domain)
            || observed != quote.association
            || message.kind() != Kind::LedgerTransferAccept
        {
            return Err(Error::Denied);
        }
        if Instant::now() >= quote.deadline {
            return Err(Error::Cancelled);
        }
        let v = message.value();
        let id = super::super::base64::decode(v["transferId"].as_str().ok_or(Error::Invalid)?, 16)?;
        if id != quote.id {
            return Err(Error::Denied);
        }
        let accepted = self.quotes.accept(
            ledger_context,
            &quote.quote,
            peer(observed),
            scalar::uint64(&v["fee"])?,
            now_milliseconds,
        )?;
        // Refuse locally unrepresentable replies before monetary commitment. Concurrent
        // accepted retries may return an earlier stored fee, so encode that actual result below.
        read::transfer_result(
            &ledger::transfer_state::TransferResult {
                order: accepted.order().clone(),
                fee: accepted.fee(),
            },
            self.config.limits,
        )?;
        let result = self
            .config
            .ledger
            .commit_transfer(ledger_context, &accepted)?;
        read::transfer_result(&result, self.config.limits).map_err(|_| Error::Unknown)
    }
    /// Drop only an original stream's unaccepted quote on a trusted close/cancellation event.
    /// # Errors
    /// Foreign local authority or Ledger grants cannot discard fee state.
    pub fn discard(
        &self,
        context: &Context<'_>,
        ledger_context: &ledger::contract::Context<'_>,
        association: Association,
    ) -> Result<(), Error> {
        tracing::trace!("Discarding only unaccepted transfer fee state");
        self.check(context, association, 0)?;
        self.quotes
            .discard(ledger_context, peer(association))
            .map_err(Error::from)
    }
    // Signed order owner follows authenticated control caller, independent of unsolicited JSON identity claims.
    fn check(&self, context: &Context<'_>, a: Association, bytes: usize) -> Result<(), Error> {
        context.check(
            &self.config.peer,
            self.config.ledger.local_identity(),
            Action::Invoke,
            bytes,
        )?;
        if a.local != self.config.ledger.local_identity()
            || a.operation.requester != a.peer
            || a.operation.stream == 0
        {
            return Err(Error::Denied);
        }
        Ok(())
    }
}
// Map only the trusted channel association into Ledger fee negotiation's opaque owner/connection/stream key.
fn peer(a: Association) -> fees::Peer {
    tracing::trace!("Binding authenticated owner to original fee stream");
    fees::Peer {
        owner: a.peer,
        connection: a.operation.connection,
        stream: a.operation.stream,
    }
}
