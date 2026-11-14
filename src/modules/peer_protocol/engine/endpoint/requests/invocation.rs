//! Requester orchestration validates quotes, reserves once and publishes results only after normal original-stream completion.
use super::*;
/// Explicit local policy deciding a separately validated immutable quote snapshot.
pub trait Consent: Send + Sync {
    /// Return an approved spending ceiling for this exact quote snapshot or refuse without acceptance.
    /// # Errors
    /// Refusal does not authorize retry or release a previously dispatched operation.
    fn approve(&self, candidate: &peer::requester::Candidate) -> Result<u64, Error>;
}
/// Explicit local ceiling policy; independent schema, original-peer and exact-hash validation still apply.
pub struct Ceiling(
    /// Approved integer-credit ceiling, checked together with the original request maximum and exact quote hash.
    pub u64,
);
impl Consent for Ceiling {
    // The application installs this policy; no peer price field creates spending permission.
    fn approve(&self, _: &peer::requester::Candidate) -> Result<u64, Error> {
        tracing::trace!("Applying explicit local quote spending policy");
        Ok(self.0)
    }
}
/// Locally selected provided work; quoted economics require separate Consent even after constructing this intent.
pub struct Invocation {
    /// Sealed selection from this original provider's directly authenticated metadata.
    pub selection: board::selection::Selection,
    /// Current direct cache and installed semantics used to revalidate acceptance.
    pub cache: Arc<PeerCache>,
    /// Complete local input; remote metadata cannot supply or replace it.
    pub input: services::value::Value,
    /// Original maximum including any explicit preagreed third-party fee/payment identity.
    pub spending: peer::requester::Spending,
    /// Separate local approval of the immutable independently validated quote.
    pub consent: Arc<dyn Consent>,
}
pub(in crate::modules::peer_protocol::engine::endpoint) struct State {
    scope: Scope,
    association: Association,
    nodes: usize,
    request: Option<peer::requester::Request>,
    cache: Option<Arc<PeerCache>>,
    consent: Arc<dyn Consent>,
    sender: Option<peer::input_transfer::InputSender>,
    receiver: Option<peer::result_receiver::ResultReceiver>,
    failure: Option<peer::failure::Failure>,
    key: Option<services::contract::OperationKey>,
}
impl State {
    // Authorize only locally chosen input/terms on the actual authenticated provider stream.
    pub(in crate::modules::peer_protocol::engine::endpoint) fn new<
        P: services::persistence::Persistence,
        L: ledger::persistence::Persistence,
    >(
        owner: &Owner<P, L>,
        scope: Scope,
        plan: Invocation,
    ) -> Result<(Self, Vec<u8>), Error> {
        tracing::trace!("Authorizing an explicit original-provider invocation intent");
        let c = &owner.config;
        if plan.selection.publisher() != scope.peer
            || plan.selection.offering().direction != board::offering::Direction::Provided
        {
            return Err(Error::Denied);
        }
        let grant = c
            .authority
            .grant(c.local, Action::Request, c.limits.state_bytes)?;
        let service = c.services.grant(
            c.local,
            services::authority::Action::Request,
            c.limits.state_bytes,
        )?;
        let request = owner.client.request(
            &context(&grant, scope.deadline),
            &services_context(&service, scope.deadline),
            scope.association(scope.local)?,
            plan.selection,
            plan.input,
            plan.spending,
        )?;
        let wire = request.bytes().to_vec();
        Ok((
            Self {
                scope,
                association: scope.association(scope.local)?,
                nodes: c.limits.nodes,
                request: Some(request),
                cache: Some(plan.cache),
                consent: plan.consent,
                sender: None,
                receiver: None,
                failure: None,
                key: None,
            },
            wire,
        ))
    }
    // Become a wanted publisher's requester only after local policy supplies input/spending and current owner terms agree.
    pub(in crate::modules::peer_protocol::engine::endpoint) fn wanted<
        P: services::persistence::Persistence,
        L: ledger::persistence::Persistence,
    >(
        owner: &Owner<P, L>,
        scope: Scope,
        selection: board::selection::Selection,
        message: &Message,
        policy: &dyn peer::wanted::Policy,
        consent: Arc<dyn Consent>,
    ) -> Result<(Self, Vec<u8>), Error> {
        tracing::trace!("Authorizing locally chosen work for an authenticated wanted invitation");
        let c = &owner.config;
        let invoke = c
            .authority
            .grant(c.local, Action::Invoke, c.limits.state_bytes)?;
        let request = c
            .authority
            .grant(c.local, Action::Request, c.limits.state_bytes)?;
        let service = c.services.grant(
            c.local,
            services::authority::Action::Request,
            c.limits.state_bytes,
        )?;
        let select = c.board_authority.grant(
            c.local,
            board::authority::Action::Select,
            c.limits.state_bytes,
        )?;
        let ic = context(&invoke, scope.deadline);
        let rc = context(&request, scope.deadline);
        let sc = services_context(&service, scope.deadline);
        let bc = board_context(&select, scope.deadline);
        let current = services::quote::BoardCurrent {
            owner: &c.board,
            context: &bc,
        };
        let association = scope.association(scope.local)?;
        let request = peer::wanted::request(
            &owner.client,
            peer::wanted::PublisherContexts {
                incoming: &ic,
                request: &rc,
                services: &sc,
                current: &current,
            },
            association,
            selection,
            message,
            policy,
        )?;
        let wire = request.bytes().to_vec();
        Ok((
            Self {
                scope,
                association,
                nodes: c.limits.nodes,
                request: Some(request),
                cache: None,
                consent,
                sender: None,
                receiver: None,
                failure: None,
                key: None,
            },
            wire,
        ))
    }
    // Independent quote checks and local consent precede one durable requester reservation and the first accept byte.
    pub(in crate::modules::peer_protocol::engine::endpoint) fn control<
        P: services::persistence::Persistence,
        L: ledger::persistence::Persistence,
    >(
        &mut self,
        owner: &Owner<P, L>,
        message: &Message,
        exchange: &mut Exchange,
        output: &mut Output,
        end_requested: &mut bool,
    ) -> Result<(), Error> {
        tracing::trace!("Binding a validated provider response to original requester work");
        let scope = self.scope;
        let association = self.association;
        let c = &owner.config;
        let invoke = c
            .authority
            .grant(c.local, Action::Invoke, c.limits.state_bytes)?;
        let ic = context(&invoke, scope.deadline);
        if message.kind() == Kind::Quote {
            let request = self.request.take().ok_or(Error::Invalid)?;
            let request_grant =
                c.authority
                    .grant(c.local, Action::Request, c.limits.state_bytes)?;
            let service = c.services.grant(
                c.local,
                services::authority::Action::Request,
                c.limits.state_bytes,
            )?;
            let coordinate = c.services.grant(
                c.local,
                services::authority::Action::Coordinate,
                c.limits.state_bytes,
            )?;
            let sc = services_context(&service, scope.deadline);
            let cc = services_context(&coordinate, scope.deadline);
            let rc = context(&request_grant, scope.deadline);
            let candidate = owner
                .client
                .validate(&ic, &sc, request, association, message)?;
            let approved = self.consent.approve(&candidate)?;
            let hash = candidate.hash();
            let consent = owner.client.consent(&rc, candidate, hash, approved)?;
            let remote = self
                .cache
                .as_ref()
                .map(|cache| cache.current(scope.deadline));
            let select = c.board_authority.grant(
                c.local,
                board::authority::Action::Select,
                c.limits.state_bytes,
            )?;
            let bc = board_context(&select, scope.deadline);
            let local = services::quote::BoardCurrent {
                owner: &c.board,
                context: &bc,
            };
            let current: &dyn services::quote::Current = match &remote {
                Some(remote) => remote,
                None => &local,
            };
            let outgoing = owner.client.reserve(
                &rc,
                peer::acceptance::Admission {
                    quote_context: &sc,
                    coordinate_context: &cc,
                    current,
                    runtime: c.runtime.as_ref(),
                },
                consent,
            )?;
            self.key = Some(outgoing.invocation().key);
            self.receiver = Some(peer::result_receiver::ResultReceiver::new(
                &ic,
                c.authority.clone(),
                &outgoing,
                c.limits,
            )?);
            self.sender = Some(peer::input_transfer::InputSender::new(outgoing, c.limits)?);
            return self.pump(exchange, output, end_requested);
        }
        if let Some(receiver) = &mut self.receiver {
            return receiver.control(&ic, association, message);
        }
        if message.kind() != Kind::Failure {
            return Err(Error::Invalid);
        }
        self.failure = Some(peer::failure::Failure::parse(message)?);
        if !*end_requested {
            exchange.end(Direction::Local)?;
            output.end();
            *end_requested = true;
        }
        Ok(())
    }
    // Detached output stays provisional under the original accepted receiver and cannot select a new financial operation.
    pub(in crate::modules::peer_protocol::engine::endpoint) fn body<
        P: services::persistence::Persistence,
        L: ledger::persistence::Persistence,
    >(
        &mut self,
        owner: &Owner<P, L>,
        record: &Record,
    ) -> Result<(), Error> {
        let scope = self.scope;
        let association = self.association;
        let c = &owner.config;
        let invoke = c
            .authority
            .grant(c.local, Action::Invoke, c.limits.state_bytes)?;
        self.receiver.as_mut().ok_or(Error::Invalid)?.body(
            &context(&invoke, scope.deadline),
            association,
            record,
        )
    }
    // Emit one bounded accept/body/marker record without advancing the source on backpressure.
    pub(in crate::modules::peer_protocol::engine::endpoint) fn pump(
        &mut self,
        exchange: &mut Exchange,
        output: &mut Output,
        end_requested: &mut bool,
    ) -> Result<(), Error> {
        tracing::trace!("Sequencing accepted requester input without repeating reservation");
        let Some(sender) = &mut self.sender else {
            return Ok(());
        };
        if output.available() == 0 || *end_requested {
            return Ok(());
        }
        let mut queue = peer::outbound::Outbound::new(output.available())?;
        match sender.enqueue_next(&mut queue, peer::record::DATA_MAX) {
            Err(Error::Limit) => return Ok(()),
            Err(error) => return Err(error),
            Ok(_) => {}
        }
        if let Some(bytes) = queue.front(usize::MAX) {
            let mut decoder = peer::record::Decoder::default();
            let (count, record) = decoder.push(bytes)?;
            if count != bytes.len() {
                return Err(Error::Invalid);
            }
            let record = record.ok_or(Error::Invalid)?;
            match &record {
                Record::Control(bytes) => {
                    exchange.control(Direction::Local, Message::parse(bytes, self.nodes)?.kind())?
                }
                Record::Body { .. } => exchange.body(Direction::Local)?,
            }
            output.enqueue(&record)?;
        }
        if sender.queued() {
            exchange.end(Direction::Local)?;
            output.end();
            *end_requested = true;
        }
        Ok(())
    }
    // Actual normal closure gates result validation and local participant commitment; interruption is handled before this call.
    pub(in crate::modules::peer_protocol::engine::endpoint) fn finish<
        P: services::persistence::Persistence,
        L: ledger::persistence::Persistence,
    >(
        mut self,
        owner: &Owner<P, L>,
    ) -> Result<services::contract::OperationKey, Error> {
        tracing::trace!("Committing a fully received original-provider result locally");
        if let Some(failure) = self.failure {
            return Err(failure.code.error());
        }
        let scope = self.scope;
        let association = self.association;
        let c = &owner.config;
        let invoke = c
            .authority
            .grant(c.local, Action::Invoke, c.limits.state_bytes)?;
        let coordinate = c.services.grant(
            c.local,
            services::authority::Action::Coordinate,
            c.limits.state_bytes,
        )?;
        let cc = services_context(&coordinate, scope.deadline);
        self.receiver.take().ok_or(Error::Invalid)?.finish(
            &context(&invoke, scope.deadline),
            association,
            true,
            &cc,
            c.runtime.as_ref(),
        )?;
        let key = self.key.ok_or(Error::Unknown)?;
        c.runtime.prepare(&cc, key)?;
        c.runtime
            .decide(&cc, key, services::runtime::Decision::Commit)?;
        c.runtime.commit(&cc, key)?;
        Ok(key)
    }
}
