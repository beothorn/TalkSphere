//! Explicit wanted policies keep publisher input/spending separate from the authenticated provider's invitation.
use super::*;
use std::sync::mpsc;
/// Application-installed publisher policy; peer invitations never install this permission or provide its input.
pub struct Publisher {
    /// Bounded local work selection; refusal must not fabricate input or financial permission.
    pub policy: Arc<dyn peer::wanted::Policy>,
    /// Independent local consent after exact quote validation, not automatic approval from publishing a Board.
    pub consent: Arc<dyn requests::Consent>,
    /// Bounded nonblocking completion observer; lost observers do not reverse durable effects.
    pub completed: mpsc::SyncSender<requests::Completion>,
}
/// Explicit prospective-provider intent against a fresh directly authenticated wanted publication.
pub struct Invitation {
    /// Sealed wanted selection; its publisher must be the actual opposite endpoint.
    pub selection: board::selection::Selection,
    /// Direct metadata and installed semantics rechecked before offer/acceptance.
    pub cache: Arc<cache::PeerCache>,
}
// Open an installed provider invitation; no input or requester spending is invented by the provider.
pub(super) fn provider<
    P: services::persistence::Persistence + 'static,
    L: ledger::persistence::Persistence + 'static,
>(
    owner: Arc<Owner<P, L>>,
    scope: Scope,
    plan: Invitation,
    output: &mut Output,
    completed: mpsc::SyncSender<requests::Completion>,
) -> Result<Box<dyn Handler>, Error> {
    tracing::trace!("Offering installed provider capability for a direct wanted publisher");
    let c = &owner.config;
    let request = c
        .authority
        .grant(c.local, Action::Request, c.limits.state_bytes)?;
    let service = c.services.grant(
        c.local,
        services::authority::Action::Invoke,
        c.limits.state_bytes,
    )?;
    let current = plan.cache.current(scope.deadline);
    let association = scope.association(scope.peer)?;
    let offered = owner.offerer.offer(
        &context(&request, scope.deadline),
        &services_context(&service, scope.deadline),
        association,
        plan.selection,
        &current,
    )?;
    let record = Record::Control(offered.bytes().to_vec());
    let mut stream = Stream {
        owner,
        scope,
        association,
        exchange: Exchange::new(Direction::Local),
        phase: Phase::Offered(Box::new(offered)),
        accepted: false,
        dispatched: false,
        end_requested: false,
        remote_current: Some(plan.cache),
        completed: Some(completed),
        failure: None,
        cause: None,
    };
    stream.emit(output, &record)?;
    Ok(Box::new(stream))
}
impl<P: services::persistence::Persistence + 'static, L: ledger::persistence::Persistence + 'static>
    Stream<P, L>
{
    // Select authoritative current wanted terms using installed requester contracts before consulting local application policy.
    pub(super) fn wanted(&mut self, message: &Message, output: &mut Output) -> Result<(), Error> {
        tracing::trace!(
            "Consulting explicit publisher policy for an authenticated wanted invitation"
        );
        let owner = self.owner.clone();
        let c = &owner.config;
        let policy = c.wanted.as_ref().ok_or(Error::Unsupported)?;
        let v = message.value();
        let service = v["service"].as_str().ok_or(Error::Invalid)?;
        let version = peer::scalar::uint32(&v["serviceVersion"])?;
        let installed = c.registry.select(service, version, false)?;
        let definition = installed.requester().definition();
        let description = definition.document.value()["description"]
            .as_str()
            .ok_or(Error::Invalid)?;
        let read = c.board_authority.grant(
            c.local,
            board::authority::Action::Read,
            c.limits.state_bytes,
        )?;
        let select = c.board_authority.grant(
            c.local,
            board::authority::Action::Select,
            c.limits.state_bytes,
        )?;
        let snapshot = c.board.read(&board_context(&read, self.scope.deadline))?;
        let selected = c.board.select(
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
        )?;
        if selected.offering().direction != board::offering::Direction::Wanted {
            return Err(Error::Denied);
        }
        self.association = self.scope.association(self.scope.local)?;
        let (state, wire) = requests::InvocationState::wanted(
            owner.as_ref(),
            self.scope,
            selected,
            message,
            policy.policy.as_ref(),
            policy.consent.clone(),
        )?;
        self.phase = Phase::Requester(Box::new(state));
        self.completed = Some(policy.completed.clone());
        self.emit(output, &Record::Control(wire))
    }
}
