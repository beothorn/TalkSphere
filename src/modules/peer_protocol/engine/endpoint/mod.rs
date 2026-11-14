//! Trusted endpoint composition over public Board, Services and Ledger contracts.
//! The stream bridge runs these handlers on workers; this module owns no sockets or keys.
use crate::modules::peer_protocol::{
    self as peer, Error,
    authority::{Action, Authority},
    contract::{Association, Context, Limits},
    engine::bridge::{Factory, Handler, Output, Scope},
    exchange::{Direction, Exchange},
    message::{Kind, Message},
    record::Record,
};
use crate::modules::{board, identity::api::IdentityId, ledger, services};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
pub mod cache;
mod emission;
mod lifecycle;
pub mod requests;
mod serving;
pub mod wanted;

/// Optional protected claim boundary installed by trusted composition, never peer metadata.
pub trait Claims: Send + Sync {
    /// Redeem through the original Ledger and protected Identity decryption grant.
    /// # Errors
    /// Missing key authority, invalid binding, storage uncertainty or bounds must remain typed.
    fn redeem(
        &self,
        association: Association,
        deadline: Instant,
        message: &Message,
    ) -> Result<Record, Error>;
}
/// Explicit owner dependencies; constructing an endpoint installs no service family.
pub struct Config<P: services::persistence::Persistence, L: ledger::persistence::Persistence> {
    /// Exact receiving identity, obtained from Identity composition.
    pub local: IdentityId,
    /// Local Peer Protocol action authority.
    pub authority: Arc<Authority>,
    /// Independent Services action authority.
    pub services: Arc<services::authority::Authority>,
    /// Current authoritative publication.
    pub board: Arc<board::state::Provider>,
    /// Board Read/Select authority issuer, retained only by trusted composition.
    pub board_authority: Arc<board::authority::Authority>,
    /// Explicitly installed family implementations and independent contract semantics.
    pub registry: Arc<services::registry::Registry>,
    /// Durable service owner, not per-stream transient state.
    pub runtime: Arc<services::runtime::Coordinator<P>>,
    /// Original local accounting owner.
    pub ledger: Arc<ledger::state::Ledger<L>>,
    /// Independent Ledger Control authority.
    pub ledger_authority: Arc<ledger::authority::Authority>,
    /// Read-only trust and economic admission policy.
    pub gate: Arc<dyn peer::admission::Gate>,
    /// Exact local issuer fee policy.
    pub transfer_policy: Arc<dyn peer::ledger_control::transfer::Policy>,
    /// Optional protected claim redemption; absence returns Unsupported.
    pub claims: Option<Arc<dyn Claims>>,
    /// Explicit local publisher work/consent policy and bounded completion observer; absence rejects wanted invitations.
    pub wanted: Option<wanted::Publisher>,
    /// Finite wire/value bounds.
    pub limits: Limits,
    /// Finite durable service owner bounds.
    pub service_limits: services::contract::Limits,
}
struct Owner<P: services::persistence::Persistence, L: ledger::persistence::Persistence> {
    config: Config<P, L>,
    issuer: peer::quote::Issuer,
    client: peer::requester::Client,
    offerer: peer::wanted::Offerer,
    authorizer: peer::proposal::Authorizer,
    checks: peer::checks::Owner,
    transfers: peer::ledger_control::transfer::Issuer<L>,
    clock: Arc<peer::checks::clock::SystemClock>,
    epoch: Instant,
}
/// Cloneable trusted incoming endpoint factory; accepted work remains in its original durable owners.
pub struct Endpoint<P: services::persistence::Persistence, L: ledger::persistence::Persistence>(
    Arc<Owner<P, L>>,
);
impl<P: services::persistence::Persistence, L: ledger::persistence::Persistence> Clone
    for Endpoint<P, L>
{
    // Cloning shares the same authority, quote bounds and durable owners; it creates no new financial domain.
    fn clone(&self) -> Self {
        tracing::trace!("Sharing a configured peer endpoint owner");
        Self(self.0.clone())
    }
}
impl<P: services::persistence::Persistence + 'static, L: ledger::persistence::Persistence + 'static>
    Endpoint<P, L>
{
    /// Bind real local owners without initializing stores, installing handlers or listening.
    /// # Errors
    /// Invalid limits, foreign owners, unavailable entropy or invalid policy fail startup.
    pub fn new(config: Config<P, L>) -> Result<Self, Error> {
        tracing::trace!("Binding trusted peer endpoint dependencies");
        config.limits.validate()?;
        config.service_limits.validate()?;
        if config.registry.local_identity() != config.local
            || config.runtime.local_identity() != config.local
            || config.ledger.local_identity() != config.local
        {
            return Err(Error::Denied);
        }
        let issuer = peer::quote::Issuer::new(
            config.local,
            config.authority.clone(),
            config.services.clone(),
            config.limits,
            config.service_limits,
            config.gate.clone(),
            Arc::new(peer::quote::SystemEntropy),
        )?;
        let authorizer = peer::proposal::Authorizer::new(
            config.local,
            config.authority.clone(),
            config.services.clone(),
            config.registry.clone(),
            config.limits,
            config.gate.clone(),
        )?;
        let checks = peer::checks::Owner::new(peer::checks::Config {
            local: config.local,
            authority: config.authority.clone(),
            services: config.services.clone(),
            registry: config.registry.clone(),
            limits: config.limits,
            gate: config.gate.clone(),
            entropy: Arc::new(peer::quote::SystemEntropy),
        })?;
        let transfers =
            peer::ledger_control::transfer::Issuer::new(peer::ledger_control::transfer::Config {
                peer: config.authority.clone(),
                authority: config.ledger_authority.clone(),
                ledger: config.ledger.clone(),
                limits: config.limits,
                policy: config.transfer_policy.clone(),
            })?;
        let offerer = peer::wanted::Offerer::new(
            config.local,
            config.authority.clone(),
            config.services.clone(),
            config.registry.clone(),
            config.limits,
            config.gate.clone(),
        )?;
        let client = peer::requester::Client::new(
            config.local,
            config.authority.clone(),
            config.services.clone(),
            config.registry.clone(),
            config.limits,
            config.service_limits,
        )?;
        let clock = Arc::new(peer::checks::clock::SystemClock::new()?);
        Ok(Self(Arc::new(Owner {
            config,
            issuer,
            client,
            offerer,
            authorizer,
            checks,
            transfers,
            clock,
            epoch: Instant::now(),
        })))
    }
}
impl<P: services::persistence::Persistence + 'static, L: ledger::persistence::Persistence + 'static>
    Factory for Endpoint<P, L>
{
    // Only authenticated incoming scopes enter the responder; local requests use explicit request factories.
    fn open(
        &self,
        scope: Scope,
        opener: Direction,
        _: &mut Output,
    ) -> Result<Box<dyn Handler>, Error> {
        tracing::trace!("Opening an authenticated incoming peer operation");
        if scope.local != self.0.config.local || opener != Direction::Remote {
            return Err(Error::Denied);
        }
        Ok(Box::new(Stream {
            owner: self.0.clone(),
            scope,
            association: scope.association(scope.peer)?,
            exchange: Exchange::new(opener),
            phase: Phase::Fresh,
            accepted: false,
            dispatched: false,
            end_requested: false,
            remote_current: None,
            completed: None,
            failure: None,
            cause: None,
        }))
    }
}
enum Quote {
    Work(Box<peer::quote::Issued>),
    Check(Box<peer::checks::Issued>),
}
enum Source {
    Work(Box<peer::result_sender::ResultSender>),
    Check(Box<peer::checks::evidence::Sender>),
    Cached(Box<peer::replay::Sender>),
}
enum Phase {
    Fresh,
    Offered(Box<peer::wanted::Offered>),
    Requester(Box<requests::InvocationState>),
    Quoted(Quote),
    Input(Box<peer::input_transfer::InputTransfer>),
    Challenge(Box<peer::checks::input::Receiver>),
    Sending(Source),
    Transfer(Box<peer::ledger_control::transfer::Quoted>),
    Terminal,
    Poisoned,
}
struct Stream<P: services::persistence::Persistence, L: ledger::persistence::Persistence> {
    owner: Arc<Owner<P, L>>,
    scope: Scope,
    association: Association,
    exchange: Exchange,
    phase: Phase,
    accepted: bool,
    dispatched: bool,
    end_requested: bool,
    remote_current: Option<Arc<cache::PeerCache>>,
    completed: Option<std::sync::mpsc::SyncSender<requests::Completion>>,
    failure: Option<peer::failure::Failure>,
    cause: Option<Error>,
}
// Construct per-call contexts with the original admission deadline, never a renewed queue-delay timeout.
fn context(grant: &peer::authority::Grant, deadline: Instant) -> Context<'_> {
    tracing::trace!("Binding a peer call to its original deadline");
    Context {
        version: 1,
        grant,
        deadline,
        cancelled: false,
    }
}
// Services grants and peer grants are independent even when their deadlines agree.
fn services_context(
    grant: &services::authority::Grant,
    deadline: Instant,
) -> services::contract::Context<'_> {
    tracing::trace!("Binding a Services call to the original peer deadline");
    services::contract::Context {
        version: 1,
        grant,
        deadline,
        cancelled: false,
    }
}
// Board metadata/selection remains protected by its own receiving owner.
fn board_context(
    grant: &board::authority::Grant,
    deadline: Instant,
) -> board::contract::Context<'_> {
    tracing::trace!("Binding an endpoint Board call");
    board::contract::Context {
        grant,
        deadline,
        cancelled: false,
    }
}
// Ledger controls cannot use a Services or Peer Protocol capability in place of accounting authority.
fn ledger_context(
    grant: &ledger::authority::Grant,
    deadline: Instant,
) -> ledger::contract::Context<'_> {
    tracing::trace!("Binding an endpoint Ledger control");
    ledger::contract::Context {
        version: 1,
        grant,
        deadline,
        cancelled: false,
    }
}

#[cfg(test)]
mod tests;

// An unknown control tag is a stream protocol violation; unsupported installed capabilities remain semantic failures after parsing.
fn parse_control(bytes: &[u8], nodes: usize) -> Result<Message, Error> {
    tracing::trace!("Parsing a control within authenticated endpoint dispatch");
    peer::message::Message::parse(bytes, nodes).map_err(|error| {
        if error == Error::Unsupported {
            Error::Invalid
        } else {
            error
        }
    })
}
