//! Early-release credit issuance through the Ledger.
//!
//! Callers: application composition, which installs a [`LedgerIssuer`] on the participant so that a committed removal issues its
//! release credits once, keyed by the Services operation. The Ledger records the issuance durably under that key, so recovery can
//! repeat the call safely.
use crate::modules::{
    identity::api::IdentityId,
    ledger::{self, authority::Grant, contract::Context, persistence::Persistence, state::Ledger},
    messaging::{error::Error, ports::ReleaseIssuer},
    services::contract::OperationKey,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// Issues release credits from a Ledger's finite local issuance.
pub struct LedgerIssuer<P: Persistence + 'static> {
    ledger: Arc<Ledger<P>>,
    grant: Grant,
    timeout: Duration,
}

impl<P: Persistence + 'static> LedgerIssuer<P> {
    /// An issuer using `grant` (Coordinate authority over this Ledger) with each call bounded by `timeout`.
    pub fn new(ledger: Arc<Ledger<P>>, grant: Grant, timeout: Duration) -> Self {
        tracing::trace!("Creating the Ledger release credit issuer");
        Self {
            ledger,
            grant,
            timeout,
        }
    }
}

impl<P: Persistence + 'static> ReleaseIssuer for LedgerIssuer<P> {
    /// Issue once per operation key; see [`Ledger::issue_release`] for the exact failure meanings.
    fn issue(&self, key: OperationKey, recipient: IdentityId, credits: u64) -> Result<(), Error> {
        tracing::trace!("Issuing release credits through the Ledger");
        let context = Context {
            version: 1,
            grant: &self.grant,
            deadline: Instant::now() + self.timeout,
            cancelled: false,
        };
        self.ledger
            .issue_release(
                &context,
                // The Ledger keeps its own copy of the same three-field operation identity.
                ledger::contract::OperationKey {
                    requester: key.requester,
                    connection: key.connection,
                    stream: key.stream,
                },
                recipient,
                credits,
            )
            .map_err(map)
    }
}

// The Messaging error with the same meaning as a Ledger failure.
fn map(error: ledger::Error) -> Error {
    tracing::trace!("Mapping a Ledger failure to Messaging");
    match error {
        ledger::Error::Invalid => Error::Invalid,
        ledger::Error::Unsupported => Error::Unsupported,
        ledger::Error::Limit => Error::Limit,
        ledger::Error::Conflict => Error::Conflict,
        ledger::Error::Insufficient => Error::Insufficient,
        ledger::Error::Storage => Error::Storage,
        ledger::Error::Unknown => Error::Unknown,
        ledger::Error::Denied | ledger::Error::Cancelled | ledger::Error::Unavailable => {
            Error::Unavailable
        }
    }
}
