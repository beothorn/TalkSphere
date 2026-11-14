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
                let message = Message::parse(&bytes, c.limits.nodes)?;
                self.exchange.control(Direction::Remote, message.kind())?;
                if let Err(error) = self.control(&message, output) {
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
        self.pump(output)
    }
    // Release unaccepted quotes and reconcile included delivery without guessing that accepted work was rolled back.
    fn closed(
        &mut self,
        outcome: crate::modules::transport::api::StreamOutcome,
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
            } else if !self.end_requested {
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
        let _ = self.exchange.close(matches!(
            outcome,
            crate::modules::transport::api::StreamOutcome::Completed
        ));
        Ok(())
    }
}

impl<P: services::persistence::Persistence + 'static, L: ledger::persistence::Persistence + 'static>
    Stream<P, L>
{
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
                self.exchange.end(Direction::Local)?;
                output.end();
                self.end_requested = true;
                return Ok(());
            }
            _ => return Err(Error::Invalid),
        };
        self.phase = Phase::Sending(source);
        self.pump(output)
    }
}
