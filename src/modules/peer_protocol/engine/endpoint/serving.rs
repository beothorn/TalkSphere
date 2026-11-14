//! Responder state transitions use authenticated scopes and real independent module authorities.
use super::*;
impl<P: services::persistence::Persistence + 'static, L: ledger::persistence::Persistence + 'static>
    Stream<P, L>
{
    // Select current provided terms using an installed contract description, never a peer-supplied description.
    fn proposal(&mut self, message: &Message, output: &mut Output) -> Result<(), Error> {
        tracing::trace!("Admitting a proposal against current installed service terms");
        let owner = self.owner.clone();
        let config = &owner.config;
        let invoke =
            config
                .authority
                .grant(config.local, Action::Invoke, config.limits.state_bytes)?;
        let service_invoke = config.services.grant(
            config.local,
            services::authority::Action::Invoke,
            config.limits.state_bytes,
        )?;
        let recover = config.services.grant(
            config.local,
            services::authority::Action::Recover,
            config.limits.state_bytes,
        )?;
        let observe = config.services.grant(
            config.local,
            services::authority::Action::Observe,
            config.limits.state_bytes,
        )?;
        let pc = context(&invoke, self.scope.deadline);
        let sc = services_context(&service_invoke, self.scope.deadline);
        if let Some(proof) = peer::replay::provider(
            &pc,
            &config.authority,
            self.association,
            message,
            peer::replay::Lookup {
                registry: &config.registry,
                recover: &services_context(&recover, self.scope.deadline),
                observe: &services_context(&observe, self.scope.deadline),
                runtime: config.runtime.as_ref(),
            },
            config.limits,
        )? {
            self.phase = Phase::Sending(Source::Cached(Box::new(peer::replay::Sender::new(
                &pc,
                &config.authority,
                proof,
                config.limits,
            )?)));
            return self.pump(output);
        }
        let v = message.value();
        let service = v["service"].as_str().ok_or(Error::Invalid)?;
        let version = peer::scalar::uint32(&v["serviceVersion"])?;
        let installed = match config.registry.select(service, version, true) {
            Ok(installed) => installed,
            Err(services::Error::Unavailable | services::Error::Unsupported) => {
                return self.refuse_code(output, peer::failure::Code::Unsupported);
            }
            Err(error) => return Err(error.into()),
        };
        let definition = installed.requester().definition();
        let description = definition.document.value()["description"]
            .as_str()
            .ok_or(Error::Invalid)?;
        let read = config.board_authority.grant(
            config.local,
            board::authority::Action::Read,
            config.limits.state_bytes,
        )?;
        let select = config.board_authority.grant(
            config.local,
            board::authority::Action::Select,
            config.limits.state_bytes,
        )?;
        let snapshot = match config
            .board
            .read(&board_context(&read, self.scope.deadline))
        {
            Ok(snapshot) => snapshot,
            Err(board::Error::Missing) => {
                return self.refuse_code(output, peer::failure::Code::NotAvailable);
            }
            Err(error) => return Err(error.into()),
        };
        let selection = match config.board.select(
            &board_context(&select, self.scope.deadline),
            &snapshot,
            &board::selection::Request {
                id: v["offeringId"].as_str().ok_or(Error::Invalid)?,
                service,
                version,
                description,
                other: self.scope.peer,
                authenticated: true,
            },
        ) {
            Ok(selection) => selection,
            Err(board::Error::Missing) => {
                return self.refuse_code(output, peer::failure::Code::NotAvailable);
            }
            Err(board::Error::Stale) => {
                return self.refuse_code(output, peer::failure::Code::TermsChanged);
            }
            Err(error) => return Err(error.into()),
        };
        if selection.offering().direction != board::offering::Direction::Provided {
            return Err(Error::Denied);
        }
        let proposal =
            owner
                .authorizer
                .authorize(&pc, &sc, self.association, selection, message)?;
        self.issue_proposal(proposal, output)
    }
    // Both provided and wanted invitations issue through one exact quote owner and role-appropriate current view.
    fn issue_proposal(
        &mut self,
        proposal: peer::proposal::Proposal,
        output: &mut Output,
    ) -> Result<(), Error> {
        tracing::trace!("Issuing exact terms for authorized provided or wanted work");
        let owner = self.owner.clone();
        let config = &owner.config;
        let invoke =
            config
                .authority
                .grant(config.local, Action::Invoke, config.limits.state_bytes)?;
        let service = config.services.grant(
            config.local,
            services::authority::Action::Invoke,
            config.limits.state_bytes,
        )?;
        let pc = context(&invoke, self.scope.deadline);
        let sc = services_context(&service, self.scope.deadline);
        let select = config.board_authority.grant(
            config.local,
            board::authority::Action::Select,
            config.limits.state_bytes,
        )?;
        let bc = board_context(&select, self.scope.deadline);
        let local = services::quote::BoardCurrent {
            owner: &config.board,
            context: &bc,
        };
        let remote = self
            .remote_current
            .as_ref()
            .map(|cache| cache.current(self.scope.deadline));
        let current: &dyn services::quote::Current = match &remote {
            Some(current) => current,
            None => &local,
        };
        let quote = match proposal.mode() {
            peer::proposal::Mode::Invoke => Quote::Work(Box::new(owner.issuer.issue(
                &pc,
                &sc,
                proposal,
                current,
                self.scope.deadline,
            )?)),
            peer::proposal::Mode::Check => Quote::Check(Box::new(owner.checks.issue(
                &pc,
                &sc,
                proposal,
                current,
                self.scope.deadline,
            )?)),
        };
        let record = Record::Control(match &quote {
            Quote::Work(q) => q.bytes().to_vec(),
            Quote::Check(q) => q.bytes().to_vec(),
        });
        self.phase = Phase::Quoted(quote);
        self.emit(output, &record)
    }
    // Validate only a proposal replying to this exact original provider invitation before quoting.
    fn wanted_proposal(&mut self, message: &Message, output: &mut Output) -> Result<(), Error> {
        tracing::trace!("Validating an original wanted publisher proposal");
        let Phase::Offered(offered) = std::mem::replace(&mut self.phase, Phase::Poisoned) else {
            return Err(Error::Invalid);
        };
        let owner = self.owner.clone();
        let c = &owner.config;
        let invoke = c
            .authority
            .grant(c.local, Action::Invoke, c.limits.state_bytes)?;
        let service = c.services.grant(
            c.local,
            services::authority::Action::Invoke,
            c.limits.state_bytes,
        )?;
        let proposal = owner.offerer.proposal(
            &context(&invoke, self.scope.deadline),
            &services_context(&service, self.scope.deadline),
            *offered,
            self.association,
            message,
        )?;
        self.issue_proposal(proposal, output)
    }
    // Consume only one exact issued quote and reserve through the original coordinator before receiving input.
    fn accept(&mut self, message: &Message) -> Result<(), Error> {
        tracing::trace!("Binding exact accepted terms to the original durable owner");
        let Phase::Quoted(quote) = std::mem::replace(&mut self.phase, Phase::Poisoned) else {
            return Err(Error::Invalid);
        };
        let owner = self.owner.clone();
        let c = &owner.config;
        let grant = c
            .authority
            .grant(c.local, Action::Invoke, c.limits.state_bytes)?;
        let invoke = c.services.grant(
            c.local,
            services::authority::Action::Invoke,
            c.limits.state_bytes,
        )?;
        let coordinate = c.services.grant(
            c.local,
            services::authority::Action::Coordinate,
            c.limits.state_bytes,
        )?;
        let select = c.board_authority.grant(
            c.local,
            board::authority::Action::Select,
            c.limits.state_bytes,
        )?;
        let pc = context(&grant, self.scope.deadline);
        let sc = services_context(&invoke, self.scope.deadline);
        let cc = services_context(&coordinate, self.scope.deadline);
        let bc = board_context(&select, self.scope.deadline);
        let local = services::quote::BoardCurrent {
            owner: &c.board,
            context: &bc,
        };
        let remote = self
            .remote_current
            .as_ref()
            .map(|cache| cache.current(self.scope.deadline));
        let current: &dyn services::quote::Current = match &remote {
            Some(current) => current,
            None => &local,
        };
        let admission = peer::acceptance::Admission {
            quote_context: &sc,
            coordinate_context: &cc,
            current,
            runtime: c.runtime.as_ref(),
        };
        self.phase = match quote {
            Quote::Work(quote) => {
                let incoming =
                    owner
                        .issuer
                        .accept(&pc, admission, *quote, self.association, message)?;
                self.accepted = true;
                Phase::Input(Box::new(peer::input_transfer::InputTransfer::new(
                    &pc,
                    c.authority.clone(),
                    incoming,
                )?))
            }
            Quote::Check(quote) => {
                let incoming =
                    owner
                        .checks
                        .accept(&pc, admission, *quote, self.association, message)?;
                self.accepted = true;
                Phase::Challenge(Box::new(peer::checks::input::Receiver::new(
                    &pc,
                    c.authority.clone(),
                    incoming,
                    owner.clock.clone(),
                )?))
            }
        };
        Ok(())
    }
    // Serve exact independent metadata/accounting controls, retaining transfer quotes until explicit acceptance.
    pub(super) fn control(&mut self, message: &Message, output: &mut Output) -> Result<(), Error> {
        tracing::trace!("Routing a validated endpoint control to its owning module");
        if let Phase::Requester(state) = &mut self.phase {
            if message.kind() == Kind::Failure {
                self.failure = Some(peer::failure::Failure::parse(message)?);
            }
            return state.control(
                self.owner.as_ref(),
                message,
                &mut self.exchange,
                output,
                &mut self.end_requested,
            );
        }
        let owner = self.owner.clone();
        let c = &owner.config;
        let grant = c
            .authority
            .grant(c.local, Action::Invoke, c.limits.state_bytes)?;
        let pc = context(&grant, self.scope.deadline);
        let record = match message.kind() {
            Kind::Propose if matches!(self.phase, Phase::Offered(_)) => {
                return self.wanted_proposal(message, output);
            }
            Kind::Failure if matches!(self.phase, Phase::Offered(_)) => {
                self.failure = Some(peer::failure::Failure::parse(message)?);
                self.phase = Phase::Terminal;
                self.exchange.end(Direction::Local)?;
                output.end();
                self.end_requested = true;
                return Ok(());
            }
            Kind::Propose if matches!(self.phase, Phase::Fresh) => {
                return self.proposal(message, output);
            }
            Kind::Accept => return self.accept(message),
            Kind::InputEnd => {
                match &mut self.phase {
                    Phase::Input(input) => input.marker(&pc, self.association, message)?,
                    Phase::Challenge(input) => {
                        input.marker_at(&pc, self.association, message, output.received_at())?
                    }
                    _ => return Err(Error::Invalid),
                }
                return Ok(());
            }
            Kind::BoardGet | Kind::ServiceGet if matches!(self.phase, Phase::Fresh) => {
                let grant = c.board_authority.grant(
                    c.local,
                    board::authority::Action::Read,
                    c.limits.state_bytes,
                )?;
                peer::metadata::respond(
                    &c.board,
                    &board_context(&grant, self.scope.deadline),
                    message,
                )?
            }
            Kind::LedgerBalance | Kind::LedgerTransferGet if matches!(self.phase, Phase::Fresh) => {
                let grant = c.ledger_authority.grant(
                    c.local,
                    ledger::authority::Action::Control,
                    c.limits.state_bytes,
                )?;
                peer::ledger_control::read::respond(
                    &pc,
                    &c.authority,
                    c.ledger.as_ref(),
                    &ledger_context(&grant, self.scope.deadline),
                    self.association,
                    message,
                    c.limits,
                )?
            }
            Kind::LedgerTransfer if matches!(self.phase, Phase::Fresh) => {
                let grant = c.ledger_authority.grant(
                    c.local,
                    ledger::authority::Action::Control,
                    c.limits.state_bytes,
                )?;
                let now =
                    u64::try_from(owner.epoch.elapsed().as_millis()).map_err(|_| Error::Limit)?;
                let remaining = u64::try_from(
                    self.scope
                        .deadline
                        .saturating_duration_since(Instant::now())
                        .as_millis(),
                )
                .map_err(|_| Error::Limit)?;
                match owner.transfers.begin(
                    &pc,
                    &ledger_context(&grant, self.scope.deadline),
                    self.association,
                    message,
                    peer::ledger_control::transfer::Time {
                        now_milliseconds: now,
                        expires_milliseconds: now.checked_add(remaining).ok_or(Error::Limit)?,
                    },
                )? {
                    peer::ledger_control::transfer::Begin::Stored(record) => record,
                    peer::ledger_control::transfer::Begin::Quote(quote) => {
                        let record = quote.record().clone();
                        self.phase = Phase::Transfer(quote);
                        return self.emit(output, &record);
                    }
                }
            }
            Kind::LedgerTransferAccept => {
                let Phase::Transfer(quote) = &self.phase else {
                    return Err(Error::Invalid);
                };
                let grant = c.ledger_authority.grant(
                    c.local,
                    ledger::authority::Action::Control,
                    c.limits.state_bytes,
                )?;
                owner.transfers.accept(
                    &pc,
                    &ledger_context(&grant, self.scope.deadline),
                    quote,
                    self.association,
                    message,
                    u64::try_from(owner.epoch.elapsed().as_millis()).map_err(|_| Error::Limit)?,
                )?
            }
            Kind::LedgerClaimRedeem if matches!(self.phase, Phase::Fresh) => c
                .claims
                .as_ref()
                .ok_or(Error::Unsupported)?
                .redeem(self.association, self.scope.deadline, message)?,
            Kind::Offer if matches!(self.phase, Phase::Fresh) => {
                return self.wanted(message, output);
            }
            _ => return Err(Error::Invalid),
        };
        self.phase = Phase::Terminal;
        self.emit(output, &record)
    }
    // A semantic refusal can claim no accepted effects only after the durable owner acknowledges an abort.
    pub(super) fn refuse(&mut self, output: &mut Output, error: Error) -> Result<(), Error> {
        tracing::trace!("Reconciling a known endpoint refusal before emitting a failure");
        self.cause = Some(error);
        self.refuse_code(output, peer::failure::Code::classify(error, false)?)
    }
    // Preserve a known protocol refusal category while independently proving abort for accepted work.
    pub(super) fn refuse_code(
        &mut self,
        output: &mut Output,
        code: peer::failure::Code,
    ) -> Result<(), Error> {
        tracing::trace!("Preserving an explicit endpoint refusal category");
        let failure = peer::failure::Failure {
            code,
            message: None,
        };
        if self.accepted {
            let c = &self.owner.config;
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
            peer::failure::abort_provider(
                &context(&invoke, self.scope.deadline),
                &c.authority,
                self.association,
                peer::failure::AbortPorts {
                    coordinate: &services_context(&coordinate, self.scope.deadline),
                    observe: &services_context(&observe, self.scope.deadline),
                    runtime: c.runtime.as_ref(),
                },
                failure.clone(),
            )?;
        }
        if !self.accepted {
            self.release_unaccepted(self.scope.deadline)?;
        }
        self.emit(output, &failure.record()?)?;
        self.phase = Phase::Terminal;
        Ok(())
    }
    // Discard only this stream's unaccepted quote capabilities, using fresh cleanup grants when appropriate.
    pub(super) fn release_unaccepted(&self, deadline: Instant) -> Result<(), Error> {
        tracing::trace!("Releasing original unaccepted endpoint quote ownership");
        let c = &self.owner.config;
        let invoke = c
            .authority
            .grant(c.local, Action::Invoke, c.limits.state_bytes)?;
        let pc = context(&invoke, deadline);
        match &self.phase {
            Phase::Quoted(Quote::Work(q)) => {
                let grant = c.services.grant(
                    c.local,
                    services::authority::Action::Invoke,
                    c.limits.state_bytes,
                )?;
                self.owner
                    .issuer
                    .discard(&pc, &services_context(&grant, deadline), q)
            }
            Phase::Quoted(Quote::Check(q)) => self.owner.checks.discard(&pc, q),
            Phase::Transfer(_) => {
                let grant = c.ledger_authority.grant(
                    c.local,
                    ledger::authority::Action::Control,
                    c.limits.state_bytes,
                )?;
                self.owner.transfers.discard(
                    &pc,
                    &ledger_context(&grant, deadline),
                    self.association,
                )
            }
            _ => Ok(()),
        }
    }
}
