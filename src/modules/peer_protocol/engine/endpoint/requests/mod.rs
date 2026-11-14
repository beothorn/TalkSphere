//! One-shot explicit local requests complete only on original normally ended authenticated streams.
use super::cache::PeerCache;
use super::*;
use std::sync::{Mutex, mpsc};
mod invocation;
pub use invocation::{Ceiling, Consent, Invocation};
/// Explicit local approval of an independently validated issuer fee; signing is not approval.
pub trait FeeConsent: Send + Sync {
    /// Approve this exact original order/fee offer or refuse without automatically retrying.
    /// # Errors
    /// Refusal prevents fresh acceptance; it does not reverse an existing transfer or service.
    fn approve(&self, offer: &peer::ledger_control::transfer_client::Offer) -> Result<(), Error>;
}
/// Explicit local exact-fee policy, separate from the signed order's maximum fee.
pub struct ExactFee(
    /// Exact locally approved fee in integer credits, including zero.
    pub u64,
);
impl FeeConsent for ExactFee {
    // Never silently substitute another fee, even when it is under the signed request ceiling.
    fn approve(&self, offer: &peer::ledger_control::transfer_client::Offer) -> Result<(), Error> {
        tracing::trace!("Checking explicit local exact transfer fee consent");
        if offer.fee() != self.0 {
            return Err(Error::Insufficient);
        }
        Ok(())
    }
}
/// Trusted local intent; this type is never deserialized from peer control messages.
pub enum Plan {
    /// Invoke selected provided work with independently validated quotes and explicit local spending consent.
    Invoke(Box<Invocation>),
    /// Fetch this peer's Board into a bounded direct-publisher cache.
    Board(Arc<PeerCache>),
    /// Fetch one definition against this peer's fresh directly retrieved Board.
    Definition {
        /// Direct-publisher cache already containing this peer's fresh Board.
        cache: Arc<PeerCache>,
        /// Exact application-selected offering ID, not an array index.
        offering: String,
    },
    /// Query only the authenticated local caller's original-issuer balance.
    Balance,
    /// Query one protected original transfer result through the authenticated issuer.
    TransferGet {
        /// Original transfer owner; issuer access remains protected by caller identity.
        owner: IdentityId,
        /// Exact lifetime transfer replay identifier.
        id: [u8; 16],
    },
    /// Submit an already signed canonical order with separate exact fee approval.
    Transfer {
        /// Canonical order, public key/signature and requested fee ceiling; never a private signer.
        signed: Box<peer::ledger_control::transfer_client::Signed>,
        /// Separate explicit local approval of the exact independently validated fee.
        consent: Arc<dyn FeeConsent>,
    },
    /// Redeem only an encrypted object and require its exact expected public claim ID.
    Claim {
        /// Exact issuer-encrypted297-byte object, without clear bearer secrets.
        envelope: Vec<u8>,
        /// Expected original public claim ID for response binding.
        id: [u8; 32],
    },
}
/// Normally ended typed observation; controls never implicitly overwrite owned credit balances.
pub enum Reply {
    /// Original requester operation committed locally; read its complete result through Services Observe authority.
    Work(services::contract::OperationKey),
    /// Directly attributed immutable metadata receipt retained by the actual cache.
    Metadata(board::cache::Remote),
    /// Original-issuer balance or opaque protected transfer observation.
    Ledger(peer::ledger_control::query::Response),
    /// Original signed transfer result with distinct fresh-versus-historical fee consent.
    Transfer(Box<peer::ledger_control::transfer_client::Completion>),
    /// Public authoritative claim result, with no bearer plaintext or implicit owned-balance update.
    Claim(peer::ledger_control::claim::Redeemed),
}
/// Bounded once-only local completion; diagnostic fields never establish rollback after abnormal closure.
pub struct Completion {
    /// Original authenticated endpoints, connection instance and stream deadline.
    pub scope: Scope,
    /// Success requires normal Transport and valid protocol/module completion; interruption is Unknown.
    pub result: Result<Reply, Error>,
    /// Exact bounded remote failure, retained as untrusted diagnostic data even on interruption.
    pub failure: Option<peer::failure::Failure>,
    /// Local callback refusal or validation cause; it does not override uncertain remote effects.
    pub cause: Option<Error>,
}
/// One consumed local intent and a single bounded completion channel; sharing does not authorize duplicate calls.
pub struct Call<P: services::persistence::Persistence, L: ledger::persistence::Persistence> {
    owner: Arc<Owner<P, L>>,
    plan: Mutex<Option<Plan>>,
    completed: mpsc::SyncSender<Completion>,
}
impl<P: services::persistence::Persistence + 'static, L: ledger::persistence::Persistence + 'static>
    Endpoint<P, L>
{
    /// Prepare one explicit intent and a capacity-one completion receiver, without opening a stream or causing effects.
    /// Use Bridge.open_with with this factory only after the intended peer connection authenticates.
    pub fn request(&self, plan: Plan) -> (Call<P, L>, mpsc::Receiver<Completion>) {
        tracing::trace!("Preparing one explicit bounded local peer request");
        let (completed, receiver) = mpsc::sync_channel(1);
        (
            Call {
                owner: self.0.clone(),
                plan: Mutex::new(Some(plan)),
                completed,
            },
            receiver,
        )
    }
}
impl<P: services::persistence::Persistence + 'static, L: ledger::persistence::Persistence + 'static>
    Factory for Call<P, L>
{
    // Consume the intent once and bind it to actual authenticated endpoints before constructing any request control.
    fn open(
        &self,
        scope: Scope,
        opener: Direction,
        output: &mut Output,
    ) -> Result<Box<dyn Handler>, Error> {
        tracing::trace!("Binding one explicit local request to its actual authenticated stream");
        let plan = self
            .plan
            .lock()
            .map_err(|_| Error::Unavailable)?
            .take()
            .ok_or(Error::Conflict)?;
        let create = || -> Result<Session<P, L>, Error> {
            if scope.local != self.owner.config.local || opener != Direction::Local {
                return Err(Error::Denied);
            }
            Session::new(
                self.owner.clone(),
                scope,
                plan,
                output,
                self.completed.clone(),
            )
        };
        match create() {
            Ok(handler) => Ok(Box::new(handler)),
            Err(error) => {
                let _ = self.completed.try_send(Completion {
                    scope,
                    result: Err(error),
                    failure: None,
                    cause: Some(error),
                });
                Err(error)
            }
        }
    }
}
enum State {
    Invoke(Box<invocation::State>),
    Metadata {
        request: peer::metadata_client::Request,
        cache: Arc<PeerCache>,
    },
    Query(peer::ledger_control::query::Request),
    Transfer {
        request: Box<peer::ledger_control::transfer_client::Request>,
        consent: Arc<dyn FeeConsent>,
    },
    Claim(peer::ledger_control::claim::Request),
}
struct Session<P: services::persistence::Persistence, L: ledger::persistence::Persistence> {
    owner: Arc<Owner<P, L>>,
    scope: Scope,
    association: Association,
    exchange: Exchange,
    state: Option<State>,
    response: Option<Message>,
    end_requested: bool,
    completed: mpsc::SyncSender<Completion>,
    failure: Option<peer::failure::Failure>,
    cause: Option<Error>,
}
impl<P: services::persistence::Persistence, L: ledger::persistence::Persistence> Session<P, L> {
    // Construct a purpose-specific request under independent Request authority and original monotonic deadline.
    fn new(
        owner: Arc<Owner<P, L>>,
        scope: Scope,
        plan: Plan,
        output: &mut Output,
        completed: mpsc::SyncSender<Completion>,
    ) -> Result<Self, Error> {
        tracing::trace!("Constructing one original-stream local request");
        let c = &owner.config;
        let grant = c
            .authority
            .grant(c.local, Action::Request, c.limits.state_bytes)?;
        let context = context(&grant, scope.deadline);
        let a = scope.association(scope.local)?;
        let (state, wire, half_close) = match plan {
            Plan::Invoke(plan) => {
                let (state, wire) = invocation::State::new(owner.as_ref(), scope, *plan)?;
                (State::Invoke(Box::new(state)), wire, false)
            }
            Plan::Board(cache) => {
                let request =
                    peer::metadata_client::Request::board(&context, c.authority.clone(), a)?;
                let bytes = request.wire().to_string().into_bytes();
                (State::Metadata { request, cache }, bytes, true)
            }
            Plan::Definition { cache, offering } => {
                let receipt = cache.receipt(scope.peer, scope.deadline)?;
                let request = peer::metadata_client::Request::definition(
                    &context,
                    c.authority.clone(),
                    a,
                    &offering,
                    receipt,
                )?;
                let bytes = request.wire().to_string().into_bytes();
                (State::Metadata { request, cache }, bytes, true)
            }
            Plan::Balance => {
                let request = peer::ledger_control::query::Request::balance(
                    &context,
                    c.authority.clone(),
                    a,
                )?;
                let bytes = request.bytes().to_vec();
                (State::Query(request), bytes, true)
            }
            Plan::TransferGet { owner, id } => {
                let request = peer::ledger_control::query::Request::transfer(
                    &context,
                    c.authority.clone(),
                    a,
                    owner,
                    id,
                )?;
                let bytes = request.bytes().to_vec();
                (State::Query(request), bytes, true)
            }
            Plan::Transfer { signed, consent } => {
                let request = peer::ledger_control::transfer_client::Request::new(
                    &context,
                    c.authority.clone(),
                    a,
                    *signed,
                )?;
                let bytes = request.bytes().to_vec();
                (
                    State::Transfer {
                        request: Box::new(request),
                        consent,
                    },
                    bytes,
                    false,
                )
            }
            Plan::Claim { envelope, id } => {
                let request = peer::ledger_control::claim::Request::new(
                    &context,
                    c.authority.clone(),
                    a,
                    &envelope,
                    id,
                )?;
                let bytes = request.bytes().to_vec();
                (State::Claim(request), bytes, true)
            }
        };
        let mut exchange = Exchange::new(Direction::Local);
        exchange.control(
            Direction::Local,
            Message::parse(&wire, c.limits.nodes)?.kind(),
        )?;
        output.enqueue(&Record::Control(wire))?;
        if half_close {
            exchange.end(Direction::Local)?;
            output.end();
        }
        Ok(Self {
            owner,
            scope,
            association: a,
            exchange,
            state: Some(state),
            response: None,
            end_requested: half_close,
            completed,
            failure: None,
            cause: None,
        })
    }
    // Validate issuer controls before separate local fee consent; never let peer payloads choose local authority.
    fn receive(&mut self, record: Record, output: &mut Output) -> Result<(), Error> {
        tracing::trace!("Validating one original peer control response");
        let Record::Control(bytes) = record else {
            self.exchange.body(Direction::Remote)?;
            let Some(State::Invoke(state)) = &mut self.state else {
                return Err(Error::Invalid);
            };
            return state.body(self.owner.as_ref(), &record);
        };
        let c = &self.owner.config;
        let message = Message::parse(&bytes, c.limits.nodes)?;
        self.exchange.control(Direction::Remote, message.kind())?;
        if message.kind() == Kind::Failure {
            self.failure = Some(peer::failure::Failure::parse(&message)?);
        }
        if let Some(State::Invoke(state)) = &mut self.state {
            return state.control(
                self.owner.as_ref(),
                &message,
                &mut self.exchange,
                output,
                &mut self.end_requested,
            );
        }

        if let Some(State::Transfer { request, consent }) = &mut self.state
            && message.kind() == Kind::LedgerTransferQuote
        {
            let invoke = c
                .authority
                .grant(c.local, Action::Invoke, c.limits.state_bytes)?;
            let offer = request.quote(
                &context(&invoke, self.scope.deadline),
                self.association,
                &message,
            )?;
            consent.approve(&offer)?;
            let grant = c
                .authority
                .grant(c.local, Action::Request, c.limits.state_bytes)?;
            let record = request.accept(&context(&grant, self.scope.deadline), &offer)?;
            let Record::Control(bytes) = &record else {
                return Err(Error::Invalid);
            };
            self.exchange.control(
                Direction::Local,
                Message::parse(bytes, c.limits.nodes)?.kind(),
            )?;
            output.enqueue(&record)?;
            self.exchange.end(Direction::Local)?;
            output.end();
            self.end_requested = true;
            return Ok(());
        }
        if message.kind() == Kind::Failure {
            self.failure = Some(peer::failure::Failure::parse(&message)?);
        }
        if !self.end_requested {
            self.exchange.end(Direction::Local)?;
            output.end();
            self.end_requested = true;
        }
        self.response = Some(message);
        Ok(())
    }
    // Consume only normal original-stream observations through the corresponding public module gate.
    fn finish(&mut self) -> Result<Reply, Error> {
        tracing::trace!("Publishing an independently validated normally ended peer observation");
        let c = &self.owner.config;
        let invoke = c
            .authority
            .grant(c.local, Action::Invoke, c.limits.state_bytes)?;
        let context = context(&invoke, self.scope.deadline);
        if matches!(self.state, Some(State::Invoke(_))) {
            let Some(State::Invoke(state)) = self.state.take() else {
                return Err(Error::Conflict);
            };
            return state.finish(self.owner.as_ref()).map(Reply::Work);
        }
        let message = self.response.take().ok_or(Error::Invalid)?;
        if message.kind() == Kind::Failure {
            return Err(peer::failure::Failure::parse(&message)?.code.error());
        }
        match self.state.take().ok_or(Error::Conflict)? {
            State::Invoke(_) => Err(Error::Conflict),
            State::Metadata { request, cache } => Ok(Reply::Metadata(cache.apply(
                request,
                &context,
                self.association,
                &message,
            )?)),
            State::Query(request) => Ok(Reply::Ledger(request.finish(
                &context,
                self.association,
                &message,
                true,
            )?)),
            State::Transfer { request, .. } => Ok(Reply::Transfer(Box::new(request.finish(
                &context,
                self.association,
                &message,
                true,
            )?))),
            State::Claim(request) => Ok(Reply::Claim(request.finish(
                &context,
                self.association,
                &message,
                true,
            )?)),
        }
    }
}
impl<P: services::persistence::Persistence + 'static, L: ledger::persistence::Persistence + 'static>
    Handler for Session<P, L>
{
    // Retain typed diagnostic causes without turning callback failure into a rollback assertion.
    fn record(&mut self, record: Record, output: &mut Output) -> Result<(), Error> {
        tracing::trace!("Receiving a one-shot local request response");
        let result = self.receive(record, output);
        if let Err(error) = result {
            self.cause = Some(error);
        }
        result
    }
    // Protocol terminal markers and both halves remain necessary before any local observer sees success.
    fn remote_end(&mut self, _: &mut Output) -> Result<(), Error> {
        tracing::trace!("Observing the original response half-close");
        self.exchange.end(Direction::Remote)
    }
    // Independent controls have no streaming producer after their bounded opening/acceptance controls.
    fn writable(&mut self, output: &mut Output) -> Result<(), Error> {
        tracing::trace!("Observing explicit request output admission");
        if let Some(State::Invoke(state)) = &mut self.state {
            state.pump(&mut self.exchange, output, &mut self.end_requested)?;
        }
        Ok(())
    }
    // A dropped caller never triggers retry or accounting changes; successful observations require actual normal closure.
    fn closed(
        &mut self,
        outcome: crate::modules::transport::api::StreamOutcome,
    ) -> Result<(), Error> {
        tracing::trace!("Completing a bounded one-shot authenticated request");
        let normal = matches!(
            outcome,
            crate::modules::transport::api::StreamOutcome::Completed
        );
        let order = self.exchange.close(normal);
        let result = if normal {
            order.and_then(|_| self.finish())
        } else {
            Err(Error::Unknown)
        };
        let _ = self.completed.try_send(Completion {
            scope: self.scope,
            result,
            failure: self.failure.take(),
            cause: self.cause,
        });
        Ok(())
    }
}
