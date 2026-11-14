//! Outgoing challenge-only checks preserve original send/receive observations across asynchronous workers.
use super::*;
/// Separate local authorization of an independently validated check quote.
pub trait CheckConsent: Send + Sync {
    /// Approve a ceiling for this immutable challenge-only quote or refuse before reservation.
    /// # Errors
    /// Refusal grants no new check attempt and does not release any earlier uncertain one.
    fn approve(&self, candidate: &peer::checks::Candidate) -> Result<u64, Error>;
}
impl CheckConsent for Ceiling {
    // The application explicitly installs a ceiling; zero is meaningful only for independently authorized included checks.
    fn approve(&self, _: &peer::checks::Candidate) -> Result<u64, Error> {
        tracing::trace!("Applying explicit local remote-check spending consent");
        Ok(self.0)
    }
}
/// Local challenge request, without requester-only expected evidence or provider-computed outcomes.
pub struct Challenge {
    /// Exact selected provided contract from the actual provider's direct metadata.
    pub selection: board::selection::Selection,
    /// Direct cache and installed semantics rechecked before acceptance.
    pub cache: Arc<PeerCache>,
    /// Only the public challenge defined by check.input, never private expected bytes.
    pub challenge: services::value::Value,
    /// Original fee/payment bound; included checks require the independently proven zero-price path.
    pub spending: peer::requester::Spending,
    /// Explicit local approval of exact validated check quote economics.
    pub consent: Arc<dyn CheckConsent>,
}
pub(super) struct State {
    scope: Scope,
    association: Association,
    request: Option<peer::checks::Request>,
    cache: Arc<PeerCache>,
    consent: Arc<dyn CheckConsent>,
    sender: Option<peer::checks::input::Sender>,
    receiver: Option<peer::checks::evidence::Receiver>,
    key: Option<services::contract::OperationKey>,
    failure: Option<peer::failure::Failure>,
    nodes: usize,
}
impl State {
    // Build an original-peer challenge proposal without private expectations, dispatch or ordinary zero-price work.
    pub(super) fn new<
        P: services::persistence::Persistence,
        L: ledger::persistence::Persistence,
    >(
        owner: &Owner<P, L>,
        scope: Scope,
        plan: Challenge,
    ) -> Result<(Self, Vec<u8>), Error> {
        tracing::trace!("Authorizing a local challenge-only remote check");
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
        let association = scope.association(scope.local)?;
        let request = owner.checks.request(
            &context(&grant, scope.deadline),
            &services_context(&service, scope.deadline),
            association,
            plan.selection,
            plan.challenge,
            plan.spending,
        )?;
        let wire = request.bytes().to_vec();
        Ok((
            Self {
                scope,
                association,
                request: Some(request),
                cache: plan.cache,
                consent: plan.consent,
                sender: None,
                receiver: None,
                key: None,
                failure: None,
                nodes: c.limits.nodes,
            },
            wire,
        ))
    }
    // Independently validate challenge economics, obtain local consent and reserve the actual check owner once.
    pub(super) fn control<
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
        tracing::trace!("Validating a challenge response under its original accepted scope");
        let c = &owner.config;
        let invoke = c
            .authority
            .grant(c.local, Action::Invoke, c.limits.state_bytes)?;
        let ic = context(&invoke, self.scope.deadline);
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
            let rc = context(&request_grant, self.scope.deadline);
            let sc = services_context(&service, self.scope.deadline);
            let cc = services_context(&coordinate, self.scope.deadline);
            let candidate = owner
                .checks
                .validate(&ic, &sc, request, self.association, message)?;
            let approved = self.consent.approve(&candidate)?;
            let hash = candidate.hash();
            let consent = owner.checks.consent(&rc, candidate, hash, approved)?;
            let current = self.cache.current(self.scope.deadline);
            let outgoing = owner.checks.reserve(
                &rc,
                peer::acceptance::Admission {
                    quote_context: &sc,
                    coordinate_context: &cc,
                    current: &current,
                    runtime: c.runtime.as_ref(),
                },
                consent,
            )?;
            self.key = Some(outgoing.invocation().key);
            self.receiver = Some(peer::checks::evidence::Receiver::new(
                &ic,
                c.authority.clone(),
                &outgoing,
                c.limits,
                owner.clock.clone(),
            )?);
            self.sender = Some(peer::checks::input::Sender::new(outgoing, c.limits)?);
            return self.pump(owner, exchange, output, end_requested);
        }
        if let Some(receiver) = &mut self.receiver {
            return receiver.control(&ic, self.association, message);
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
    // Complete evidence remains provisional and body fragments carry no provider-calculated correctness conclusion.
    pub(super) fn body<
        P: services::persistence::Persistence,
        L: ledger::persistence::Persistence,
    >(
        &mut self,
        owner: &Owner<P, L>,
        record: &Record,
    ) -> Result<(), Error> {
        let c = &owner.config;
        let invoke = c
            .authority
            .grant(c.local, Action::Invoke, c.limits.state_bytes)?;
        self.receiver.as_mut().ok_or(Error::Invalid)?.body(
            &context(&invoke, self.scope.deadline),
            self.association,
            record,
        )
    }
    // Record the original final-input drain only after all controls drained, before requesting requester END.
    pub(super) fn pump<
        P: services::persistence::Persistence,
        L: ledger::persistence::Persistence,
    >(
        &mut self,
        owner: &Owner<P, L>,
        exchange: &mut Exchange,
        output: &mut Output,
        end_requested: &mut bool,
    ) -> Result<(), Error> {
        tracing::trace!("Sequencing check acceptance and original complete challenge send timing");
        let Some(sender) = &mut self.sender else {
            return Ok(());
        };
        if *end_requested || output.available() == 0 {
            return Ok(());
        }
        let mut queue = peer::outbound::Outbound::new(output.available())?;
        let advanced = match sender.enqueue_next(&mut queue) {
            Err(Error::Limit) => return Ok(()),
            Err(error) => return Err(error),
            Ok(advanced) => advanced,
        };
        if let Some(bytes) = queue.front(usize::MAX) {
            let mut decoder = peer::record::Decoder::default();
            let (count, record) = decoder.push(bytes)?;
            if count != bytes.len() {
                return Err(Error::Invalid);
            }
            let record = record.ok_or(Error::Invalid)?;
            let Record::Control(bytes) = &record else {
                return Err(Error::Invalid);
            };
            exchange.control(Direction::Local, Message::parse(bytes, self.nodes)?.kind())?;
            output.enqueue(&record)?;
        }
        if !advanced {
            let c = &owner.config;
            let grant = c.services.grant(
                c.local,
                services::authority::Action::Coordinate,
                c.limits.state_bytes,
            )?;
            let drained = output.drained_at().ok_or(Error::Conflict)?;
            sender.mark_sent_at(
                output.queue(),
                &services_context(&grant, self.scope.deadline),
                c.runtime.as_ref(),
                owner.clock.stamp_at(drained)?,
            )?;
            exchange.end(Direction::Local)?;
            output.end();
            *end_requested = true;
        }
        Ok(())
    }
    // Original complete-response timing gates evidence and local accounting; private correctness remains inside installed requester rules.
    pub(super) fn finish<
        P: services::persistence::Persistence,
        L: ledger::persistence::Persistence,
    >(
        mut self,
        owner: &Owner<P, L>,
        completed_at: Instant,
    ) -> Result<services::contract::OperationKey, Error> {
        tracing::trace!(
            "Committing complete evidence using its original local receive observation"
        );
        if let Some(failure) = self.failure {
            return Err(failure.code.error());
        }
        let c = &owner.config;
        let invoke = c
            .authority
            .grant(c.local, Action::Invoke, c.limits.state_bytes)?;
        let coordinate = c.services.grant(
            c.local,
            services::authority::Action::Coordinate,
            c.limits.state_bytes,
        )?;
        let cc = services_context(&coordinate, self.scope.deadline);
        self.receiver.take().ok_or(Error::Invalid)?.finish_at(
            &context(&invoke, self.scope.deadline),
            self.association,
            true,
            &cc,
            c.runtime.as_ref(),
            owner.clock.stamp_at(completed_at)?,
        )?;
        let key = self.key.ok_or(Error::Unknown)?;
        c.runtime.prepare(&cc, key)?;
        c.runtime
            .decide(&cc, key, services::runtime::Decision::Commit)?;
        c.runtime.commit(&cc, key)?;
        Ok(key)
    }
}
