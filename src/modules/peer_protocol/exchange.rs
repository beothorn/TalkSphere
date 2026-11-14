//! Direction-aware exchange ordering; it proves neither application permission nor body/content validity.
use super::{Error, message::Kind};
/// Direction relative to the local protocol owner, independent of TCP and application roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Submitted by trusted local composition.
    Local,
    /// Received from the authenticated opposite stream endpoint.
    Remote,
}
impl Direction {
    // Obtain the opposite endpoint without deriving requester roles from TCP direction.
    fn other(self) -> Self {
        match self {
            Self::Local => Self::Remote,
            Self::Remote => Self::Local,
        }
    }
    // Use a fixed two-entry direction table, never peer-controlled array indices.
    fn index(self) -> usize {
        match self {
            Self::Local => 0,
            Self::Remote => 1,
        }
    }
}
/// Terminal wire observation, separate from Ledger payment and family verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Complete valid-order success with normal END in both directions.
    Complete,
    /// Complete explicit failure or pre-acceptance interruption.
    Failure,
    /// Accepted work may have effects; recovery must consult local owners.
    Unknown,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Start,
    Wanted,
    Quote,
    Accept,
    Input,
    InputEnd,
    Result,
    Output,
    Terminal,
    Failed,
    Reply(Kind),
    FeeQuote,
    FeeAccept,
    TransferResult,
}
/// One fresh stream's wire ordering; callers independently enforce grants, values, pricing and participant decisions.
pub struct Exchange {
    opener: Direction,
    requester: Direction,
    provider: Direction,
    phase: Phase,
    ended: [bool; 2],
    accepted: bool,
    closed: bool,
    poisoned: bool,
    cached: bool,
    ledger_effect_possible: bool,
}
impl Exchange {
    /// Start a fresh exchange with its trusted Transport stream opener direction.
    pub fn new(opener: Direction) -> Self {
        tracing::trace!("Starting direction-aware peer exchange ordering");
        Self {
            opener,
            requester: opener,
            provider: opener.other(),
            phase: Phase::Start,
            ended: [false; 2],
            accepted: false,
            closed: false,
            poisoned: false,
            cached: false,
            ledger_effect_possible: false,
        }
    }
    /// Observe a structurally validated control; this ordering model does not prove its semantic authorization.
    /// # Errors
    /// Illegal sender/order or use after END permanently poisons this stream; no other exchange is changed.
    pub fn control(&mut self, direction: Direction, kind: Kind) -> Result<(), Error> {
        tracing::trace!("Advancing validated peer control ordering");
        let result = self.transition(direction, kind);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    // Match every legal control transition, keeping independent directions and immutable accepted state.
    fn transition(&mut self, direction: Direction, kind: Kind) -> Result<(), Error> {
        tracing::trace!("Checking peer message sender and phase");
        self.live(direction)?;
        if kind == Kind::Failure {
            if matches!(self.phase, Phase::Start | Phase::Terminal | Phase::Failed) {
                return Err(Error::Invalid);
            }
            if direction != self.provider
                && !(self.phase == Phase::Wanted && direction == self.requester)
            {
                return Err(Error::Invalid);
            }
            self.phase = Phase::Failed;
            return Ok(());
        }
        let next = match self.phase {
            Phase::Start if direction == self.opener => match kind {
                Kind::BoardGet => Phase::Reply(Kind::Board),
                Kind::ServiceGet => Phase::Reply(Kind::Definition),
                Kind::Propose => Phase::Quote,
                Kind::Offer => {
                    self.provider = self.opener;
                    self.requester = self.opener.other();
                    Phase::Wanted
                }
                Kind::LedgerBalance => Phase::Reply(Kind::LedgerBalanceResult),
                Kind::LedgerTransferGet => Phase::Reply(Kind::LedgerTransferResult),
                Kind::LedgerClaimRedeem => {
                    self.ledger_effect_possible = true;
                    Phase::Reply(Kind::LedgerClaimResult)
                }
                Kind::LedgerTransfer => {
                    self.ledger_effect_possible = true;
                    Phase::FeeQuote
                }
                _ => return Err(Error::Invalid),
            },
            Phase::Wanted if direction == self.requester && kind == Kind::Propose => Phase::Quote,
            Phase::Quote if direction == self.provider && kind == Kind::Quote => Phase::Accept,
            Phase::Accept if direction == self.requester && kind == Kind::Accept => {
                self.accepted = true;
                Phase::Input
            }
            Phase::Input if direction == self.requester && kind == Kind::InputEnd => {
                Phase::InputEnd
            }
            Phase::Result if direction == self.provider && kind == Kind::Result => Phase::Output,
            Phase::Output if direction == self.provider && kind == Kind::ResultEnd => {
                Phase::Terminal
            }
            Phase::Reply(expected) if direction == self.provider && kind == expected => {
                Phase::Terminal
            }
            Phase::FeeQuote if direction == self.provider => match kind {
                Kind::LedgerTransferQuote => Phase::FeeAccept,
                Kind::LedgerTransferResult => Phase::Terminal,
                _ => return Err(Error::Invalid),
            },
            Phase::FeeAccept
                if direction == self.requester && kind == Kind::LedgerTransferAccept =>
            {
                self.accepted = true;
                Phase::TransferResult
            }
            Phase::TransferResult
                if direction == self.provider && kind == Kind::LedgerTransferResult =>
            {
                Phase::Terminal
            }
            _ => return Err(Error::Invalid),
        };
        self.phase = next;
        Ok(())
    }
    /// Permit local cached result only with original-owner family-approved committed response proof.
    /// # Errors
    /// Unrelated early results and wrong sender/order still poison this stream.
    pub fn cached_local(
        &mut self,
        _proof: &crate::modules::services::replay::Cached,
    ) -> Result<(), Error> {
        tracing::trace!("Advancing an explicitly validated local cached result");
        self.cached_result(Direction::Local)
    }
    /// Permit remote cached result only for an explicit requester-authorized original family retry.
    /// # Errors
    /// Missing matching proposal or wrong result direction remains a stream protocol error.
    pub fn cached_remote(
        &mut self,
        _proof: &crate::modules::services::replay::Retry,
    ) -> Result<(), Error> {
        tracing::trace!("Advancing an explicitly authorized remote cached result");
        self.cached_result(Direction::Remote)
    }
    // This exception requires opaque semantic proof at its public entry points, never a boolean decoded from JSON.
    fn cached_result(&mut self, direction: Direction) -> Result<(), Error> {
        tracing::trace!("Checking the narrow cached-result ordering exception");
        if self.live(direction).is_err() || self.phase != Phase::Quote || direction != self.provider
        {
            self.poisoned = true;
            return Err(Error::Invalid);
        }
        self.cached = true;
        self.accepted = true;
        self.phase = Phase::Output;
        Ok(())
    }
    /// Check whether a binary record is legal here; the owning Receiver additionally verifies every declared byte.
    /// # Errors
    /// Bodies outside accepted input/output phases poison this stream.
    pub fn body(&mut self, direction: Direction) -> Result<(), Error> {
        let result = self.live(direction).and_then(|()| {
            if (self.phase == Phase::Input && direction == self.requester)
                || (self.phase == Phase::Output && direction == self.provider)
            {
                Ok(())
            } else {
                Err(Error::Invalid)
            }
        });
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    /// Observe an endpoint's normal END, separately from final stream closure.
    /// # Errors
    /// Missing markers, premature transfer-owner END and duplicate END poison this stream.
    pub fn end(&mut self, direction: Direction) -> Result<(), Error> {
        tracing::trace!("Checking direction-specific peer exchange END");
        let result = self.end_direction(direction);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    // Requester half-close may precede a metadata reply, but invocation input requires input.end first.
    fn end_direction(&mut self, direction: Direction) -> Result<(), Error> {
        tracing::trace!("Validating peer half-close against message phase");
        self.live(direction)?;
        let allowed = if direction == self.requester {
            match self.phase {
                Phase::Reply(_) => true,
                Phase::Output if self.cached => true,
                Phase::InputEnd => {
                    self.phase = Phase::Result;
                    true
                }
                Phase::Terminal | Phase::Failed | Phase::TransferResult => true,
                _ => false,
            }
        } else {
            matches!(self.phase, Phase::Terminal | Phase::Failed)
        };
        if !allowed {
            return Err(Error::Invalid);
        }
        self.ended[direction.index()] = true;
        Ok(())
    }
    /// Interpret final Transport closure only after directional markers/order and independent content validation.
    /// # Errors
    /// Normal close without both legal ENDs is Invalid; use after poison/close is Conflict.
    /// The accepted flag remains available even when a protocol error requires financial uncertainty.
    pub fn close(&mut self, normal: bool) -> Result<Outcome, Error> {
        tracing::trace!("Classifying final peer exchange completion");
        if self.closed || self.poisoned {
            return Err(Error::Conflict);
        }
        self.closed = true;
        if !normal {
            return Ok(if self.accepted || self.ledger_effect_possible {
                Outcome::Unknown
            } else {
                Outcome::Failure
            });
        }
        if !self.ended.iter().all(|end| *end) {
            return Err(Error::Invalid);
        }
        match self.phase {
            Phase::Terminal => Ok(Outcome::Complete),
            Phase::Failed => Ok(Outcome::Failure),
            _ => Err(Error::Invalid),
        }
    }
    /// Read possible accepted-effect state without treating a framing failure as rollback.
    pub fn accepted(&self) -> bool {
        tracing::trace!("Reading accepted peer exchange uncertainty guard");
        self.accepted
    }
    /// Read current application roles; wanted offers deliberately reverse the opener relationship.
    pub fn roles(&self) -> (Direction, Direction) {
        tracing::trace!("Reading explicit peer application directions");
        (self.requester, self.provider)
    }
    // Keep illegal transitions local and permanently reject further use of the poisoned exchange.
    fn live(&self, direction: Direction) -> Result<(), Error> {
        if self.closed || self.poisoned {
            return Err(Error::Conflict);
        }
        if self.ended[direction.index()] {
            return Err(Error::Invalid);
        }
        Ok(())
    }
}
