//! Accepted execution and closure preserve durable ownership across asynchronous Transport loss.
use super::*;
use peer::engine::bridge::Handler;
impl<P: services::persistence::Persistence + 'static, L: ledger::persistence::Persistence + 'static>
    Handler for Stream<P, L>
{
    // Framing and typed ordering precede semantic admission; bodies never select handlers or requester roles.
    fn record(&mut self, record: Record, output: &mut Output) -> Result<(), Error> {
        if matches!(record, Record::Control(_)) {
            tracing::trace!("Applying an authenticated endpoint control");
        }
        // Fragment entry logs are omitted; the stream lifecycle and bounded controls identify this operation.
        let owner = self.owner.clone();
        let c = &owner.config;
        match record {
            Record::Control(bytes) => {
                let message = parse_control(&bytes, c.limits.nodes)?;
                self.exchange.control(Direction::Remote, message.kind())?;
                if let Err(error) = self.control(&message, output) {
                    self.cause = Some(error);
                    if matches!(self.phase, Phase::Requester(_)) {
                        return Err(error);
                    }
                    let code = match (message.kind(), error) {
                        (Kind::Accept, Error::Conflict) => peer::failure::Code::TermsChanged,
                        (Kind::Accept | Kind::LedgerTransferAccept, Error::Cancelled) => {
                            peer::failure::Code::Conflict
                        }
                        (Kind::LedgerTransfer, Error::Insufficient) => {
                            peer::failure::Code::PriceRejected
                        }
                        (_, error) => peer::failure::Code::classify(error, false)?,
                    };
                    self.refuse_code(output, code)?;
                }
            }
            record @ Record::Body { .. } => {
                self.exchange.body(Direction::Remote)?;
                if let Phase::Requester(state) = &mut self.phase {
                    return state.body(owner.as_ref(), &record);
                }
                let grant = c
                    .authority
                    .grant(c.local, Action::Invoke, c.limits.state_bytes)?;
                let Phase::Input(input) = &mut self.phase else {
                    return Err(Error::Invalid);
                };
                input.body(
                    &context(&grant, self.scope.deadline),
                    self.association,
                    &record,
                )?;
            }
        }
        Ok(())
    }
    // Complete requester input before dispatch, then commit both local participants before creating a result sender.
    fn remote_end(&mut self, output: &mut Output) -> Result<(), Error> {
        tracing::trace!("Completing provider input and coordinating durable execution");
        self.exchange.end(Direction::Remote)?;
        if matches!(self.phase, Phase::Requester(_)) {
            return Ok(());
        }
        match self.dispatch_input(output) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.refuse(output, error)?;
                self.exchange.end(Direction::Local)?;
                output.end();
                self.end_requested = true;
                Ok(())
            }
        }
    }
    // Refill only from immutable committed sources; backpressure does not rerun family work.
    fn writable(&mut self, output: &mut Output) -> Result<(), Error> {
        tracing::trace!("Refilling a committed endpoint result");
        if let Phase::Requester(state) = &mut self.phase {
            return state.pump(&mut self.exchange, output, &mut self.end_requested);
        }
        self.pump(output)
    }
    // Release unaccepted quotes and reconcile included delivery without guessing that accepted work was rolled back.
    fn closed(
        &mut self,
        outcome: crate::modules::transport::api::StreamOutcome,
    ) -> Result<(), Error> {
        self.reconcile(outcome, None)
    }
    // Preserve original closure facts for check delivery and wanted completion before deferred cleanup.
    fn closed_observed(&mut self, closure: peer::engine::bridge::Closure) -> Result<(), Error> {
        tracing::trace!("Reconciling original endpoint closure observations");
        self.reconcile(closure.outcome, Some(closure.local_end_submitted))
    }
}

impl<P: services::persistence::Persistence + 'static, L: ledger::persistence::Persistence + 'static>
    Stream<P, L>
{
    // Complete requester or provider roles under the same original financial owner, never from opener direction.
    fn reconcile(
        &mut self,
        outcome: crate::modules::transport::api::StreamOutcome,
        local_end_submitted: Option<bool>,
    ) -> Result<(), Error> {
        tracing::trace!("Reconciling endpoint role and actual terminal observations");
        let normal = matches!(
            outcome,
            crate::modules::transport::api::StreamOutcome::Completed
        );
        let order = self.exchange.close(normal);
        if matches!(self.phase, Phase::Requester(_)) {
            let Phase::Requester(state) = std::mem::replace(&mut self.phase, Phase::Poisoned)
            else {
                return Err(Error::Conflict);
            };
            let result = if normal {
                order
                    .and_then(|_| state.finish(self.owner.as_ref()))
                    .map(requests::Reply::Work)
            } else {
                Err(Error::Unknown)
            };
            if let Some(completed) = self.completed.take() {
                let _ = completed.try_send(requests::Completion {
                    scope: self.scope,
                    result,
                    failure: self.failure.take(),
                    cause: self.cause,
                });
            }
            return Ok(());
        }
        let result = self.reconcile_provider(outcome, local_end_submitted);
        if let Some(completed) = self.completed.take() {
            let reply = if !normal {
                Err(Error::Unknown)
            } else if let Some(failure) = &self.failure {
                Err(failure.code.error())
            } else if let Some(error) = self.cause {
                Err(error)
            } else {
                order.and_then(|_| match &self.phase {
                    Phase::Sending(Source::Cached(sender)) => {
                        Ok(requests::Reply::Work(sender.proof().invocation().key))
                    }
                    Phase::Sending(Source::Check(_)) => Ok(requests::Reply::Check(
                        peer::acceptance::peer(self.association).key,
                    )),
                    Phase::Sending(Source::Work(_)) => Ok(requests::Reply::Work(
                        peer::acceptance::peer(self.association).key,
                    )),
                    _ => Err(Error::Unknown),
                })
            };
            let reply = result.as_ref().map_err(|error| *error).and(reply);
            let _ = completed.try_send(requests::Completion {
                scope: self.scope,
                result: reply,
                failure: self.failure.take(),
                cause: self.cause,
            });
        }
        result
    }
    // Provider cleanup retains dispatched effects and distinguishes known-unsent END from ambiguous delivery.
    fn reconcile_provider(
        &mut self,
        outcome: crate::modules::transport::api::StreamOutcome,
        local_end_submitted: Option<bool>,
    ) -> Result<(), Error> {
        tracing::trace!("Reconciling endpoint closure with original durable owners");
        let owner = self.owner.clone();
        let c = &owner.config;
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(5))
            .ok_or(Error::Limit)?;
        let coordinate = c.services.grant(
            c.local,
            services::authority::Action::Coordinate,
            c.limits.state_bytes,
        )?;
        let cc = services_context(&coordinate, deadline);
        self.release_unaccepted(deadline)?;
        if let Phase::Sending(Source::Check(_)) = &self.phase {
            let delivery = if matches!(
                outcome,
                crate::modules::transport::api::StreamOutcome::Completed
            ) {
                services::checks::Delivery::Complete
            } else if local_end_submitted == Some(false)
                || (!self.end_requested && local_end_submitted.is_none())
            {
                services::checks::Delivery::Incomplete
            } else {
                services::checks::Delivery::Unknown
            };
            // Normal bridge closure proves all queued records and local END were written; no receipt is invented.
            match c.runtime.check_delivery(
                &cc,
                peer::acceptance::peer(self.association).key,
                delivery,
            ) {
                Ok(()) | Err(services::Error::Unsupported) => {}
                Err(error) => return Err(error.into()),
            }
        }
        if self.accepted && !self.dispatched {
            let key = peer::acceptance::peer(self.association).key;
            c.runtime
                .decide(&cc, key, services::runtime::Decision::Abort)?;
            c.runtime.abort(&cc, key)?;
        }
        // Exchange failure may already be poisoned; that cannot erase accounting or create a successful result.
        Ok(())
    }
    // Run complete input through its original owner; caller emits failure only after an acknowledged uncommitted abort.
    fn dispatch_input(&mut self, output: &mut Output) -> Result<(), Error> {
        tracing::trace!("Dispatching complete endpoint input and retaining committed output");
        let owner = self.owner.clone();
        let c = &owner.config;
        let invoke = c
            .authority
            .grant(c.local, Action::Invoke, c.limits.state_bytes)?;
        let coordinate = c.services.grant(
            c.local,
            services::authority::Action::Coordinate,
            c.limits.state_bytes,
        )?;
        let observe = c.services.grant(
            c.local,
            services::authority::Action::Observe,
            c.limits.state_bytes,
        )?;
        let pc = context(&invoke, self.scope.deadline);
        let cc = services_context(&coordinate, self.scope.deadline);
        let oc = services_context(&observe, self.scope.deadline);
        let phase = std::mem::replace(&mut self.phase, Phase::Poisoned);
        let key = peer::acceptance::peer(self.association).key;
        let source = match phase {
            Phase::Input(input) => {
                input
                    .end(&pc, self.association)?
                    .dispatch(&cc, c.runtime.as_ref())?;
                self.dispatched = true;
                c.runtime.stage(&cc, key)?;
                c.runtime.prepare(&cc, key)?;
                c.runtime
                    .decide(&cc, key, services::runtime::Decision::Commit)?;
                c.runtime.commit(&cc, key)?;
                Source::Work(Box::new(peer::result_sender::ResultSender::new(
                    &pc,
                    &c.authority,
                    &oc,
                    c.runtime.as_ref(),
                    self.association,
                    c.limits,
                )?))
            }
            Phase::Challenge(input) => {
                let dispatched = input.end(&pc, self.association, &cc, c.runtime.as_ref())?;
                self.dispatched = true;
                c.runtime.stage(&cc, key)?;
                c.runtime.prepare(&cc, key)?;
                c.runtime
                    .decide(&cc, key, services::runtime::Decision::Commit)?;
                c.runtime.commit(&cc, key)?;
                Source::Check(Box::new(peer::checks::evidence::Sender::new(
                    &pc,
                    &c.authority,
                    &oc,
                    c.runtime.as_ref(),
                    dispatched,
                    c.limits,
                )?))
            }
            Phase::Sending(source) => {
                self.phase = Phase::Sending(source);
                return self.pump(output);
            }
            Phase::Terminal => {
                self.phase = Phase::Terminal;
                if !self.end_requested {
                    self.exchange.end(Direction::Local)?;
                    output.end();
                    self.end_requested = true;
                }
                return Ok(());
            }
            _ => return Err(Error::Invalid),
        };
        self.phase = Phase::Sending(source);
        self.pump(output)
    }
}
