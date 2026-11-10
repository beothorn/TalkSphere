//! Original-peer provided proposals bind exact selected contracts before family assessment or large body transfer.
use super::{
    Error,
    admission::{Gate, Roles},
    authority::{Action, Authority},
    base64,
    contract::{Association, Context, Limits},
    message::{Kind, Message},
    scalar,
    value::Pending,
};
use crate::modules::{
    board::{offering::Direction, selection::Selection},
    identity::{api::IdentityId, text},
    services::{self, authorization::Authorized, registry::Registry, runtime::Payment},
};
use std::sync::Arc;
/// Supported proposal purpose; remote check eligibility is separately validated by installed check rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Ordinary paid work.
    Invoke,
    /// Challenge-only remote evidence.
    Check,
}
/// Immutable validated proposal, still neither accepted nor reserved.
pub struct Proposal {
    pub(crate) authorized: Authorized,
    pub(crate) input: Pending,
    pub(crate) mode: Mode,
    pub(crate) maximum: u64,
    pub(crate) payment: Payment,
}
impl Proposal {
    /// Borrow the owner-authorized exact selected implementation and peer context without granting execution.
    pub fn authorized(&self) -> &Authorized {
        tracing::trace!("Borrowing authorized proposal selection");
        &self.authorized
    }
    /// Borrow bounded unresolved metadata without exposing provisional complete input.
    pub fn input(&self) -> &Pending {
        tracing::trace!("Borrowing validated proposal input metadata");
        &self.input
    }
    /// Read explicit request spending bound, inclusive of any preagreed issuer fee.
    pub fn maximum(&self) -> u64 {
        tracing::trace!("Reading immutable proposal spending ceiling");
        self.maximum
    }
    /// Read the authorized invocation/check purpose.
    pub fn mode(&self) -> Mode {
        tracing::trace!("Reading validated proposal purpose");
        self.mode
    }
    /// Copy the exact original optional transfer/fee association.
    pub fn payment(&self) -> Payment {
        tracing::trace!("Reading original proposal payment association");
        self.payment
    }
}
/// Receiving owner pins its independent Peer Protocol/Services domains, registry, policy and finite limits.
pub struct Authorizer {
    local: IdentityId,
    authority: Arc<Authority>,
    services: Arc<services::authority::Authority>,
    registry: Arc<Registry>,
    limits: Limits,
    gate: Arc<dyn Gate>,
}
impl Authorizer {
    /// Configure the exact receiving identity and explicitly installed implementations/policy.
    /// # Errors
    /// Invalid limits or a differently owned registry fail before any proposal.
    pub fn new(
        local: IdentityId,
        authority: Arc<Authority>,
        services: Arc<services::authority::Authority>,
        registry: Arc<Registry>,
        limits: Limits,
        gate: Arc<dyn Gate>,
    ) -> Result<Self, Error> {
        tracing::trace!("Configuring provided proposal authorization");
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
        })
    }
    /// Validate a provided proposal against a current owner-issued selection and trusted original stream association.
    /// # Errors
    /// Wrong authority/roles/publisher/issuer, unsupported capabilities/preconditions, bad metadata or policy are refused without effects.
    pub fn authorize(
        &self,
        context: &Context<'_>,
        services_context: &services::contract::Context<'_>,
        association: Association,
        selection: Selection,
        message: &Message,
    ) -> Result<Proposal, Error> {
        tracing::trace!("Authorizing provided work against exact selected terms");
        context.check(
            &self.authority,
            self.local,
            Action::Invoke,
            message.bytes().len(),
        )?;
        if message.kind() != Kind::Propose
            || association.local != self.local
            || selection.offering().direction != Direction::Provided
        {
            return Err(Error::Denied);
        }
        let v = message.value();
        let o = selection.offering();
        let (requester, provider, issuer) = selection.roles();
        if selection.publisher() != self.local
            || provider != self.local
            || requester != association.peer
            || association.operation.requester != requester
            || association.operation.stream == 0
        {
            return Err(Error::Denied);
        }
        if v["offeringId"].as_str() != Some(o.id.as_str())
            || v["service"].as_str() != Some(o.service.as_str())
            || scalar::uint32(&v["serviceVersion"])? != o.version
        {
            return Err(Error::Conflict);
        }
        if identity(&v["publisherId"])? != selection.publisher()
            || identity(&v["creditIssuer"])? != issuer
        {
            return Err(Error::Denied);
        }
        let mode = match v["mode"].as_str() {
            Some("invoke") => Mode::Invoke,
            Some("check") => Mode::Check,
            _ => return Err(Error::Unsupported),
        };
        let maximum = scalar::uint64(&v["maximumPrice"])?;
        if mode == Mode::Invoke && maximum == 0 {
            return Err(Error::Insufficient);
        }
        let payment = if issuer != requester && issuer != provider {
            let transfer = base64::decode(v["transferId"].as_str().ok_or(Error::Invalid)?, 16)?
                .try_into()
                .map_err(|_| Error::Invalid)?;
            Payment {
                transfer: Some(transfer),
                fee: Some(scalar::uint64(&v["transferFee"])?),
            }
        } else {
            if v.get("transferId").is_some() || v.get("transferFee").is_some() {
                return Err(Error::Invalid);
            }
            Payment::default()
        };
        let peer = services::handler::Peer {
            local: self.local,
            remote: association.peer,
            key: services::contract::OperationKey {
                requester,
                connection: association.operation.connection,
                stream: association.operation.stream,
            },
        };
        let authorized = services::authorization::authorize(
            services_context,
            &self.services,
            self.local,
            &self.registry,
            selection,
            peer,
            None,
        )?;
        let descriptor = if mode == Mode::Check {
            if authorized.implementation().requester().checks().is_none() {
                return Err(Error::Unsupported);
            }
            &authorized.selection().definition().document.value()["check"]["input"]
        } else {
            &authorized.selection().definition().document.value()["input"]
        };
        let input = Pending::parse(
            &v["input"],
            descriptor,
            self.limits,
            authorized.implementation().requester().nonfinite(),
        )?;
        if mode == Mode::Invoke {
            self.gate.check(
                context,
                Roles {
                    requester,
                    provider,
                    issuer,
                },
            )?;
        }
        Ok(Proposal {
            authorized,
            input,
            mode,
            maximum,
            payment,
        })
    }
}
// Parse already canonical control identity text while independently rejecting absent/wrong values.
fn identity(value: &serde_json::Value) -> Result<IdentityId, Error> {
    tracing::trace!("Checking proposal identity binding");
    text::parse(value.as_str().ok_or(Error::Invalid)?).map_err(|_| Error::Invalid)
}
