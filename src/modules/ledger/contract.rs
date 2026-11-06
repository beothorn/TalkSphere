//! Exact operation identity and bounded caller context, independent of peer JSON.
use super::{
    Error,
    authority::{Action, Authority, Grant},
};
use crate::modules::identity::api::IdentityId;
use std::time::Instant;

/// Logical direct-call contract version, separate from peer wire versions.
pub const CONTRACT_VERSION: u32 = 1;

/// Durable identity of one accepted attempt, never inferred from identical input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct OperationKey {
    /// Authenticated requester identity.
    pub requester: IdentityId,
    /// Unique connection instance assigned by trusted composition.
    pub connection: [u8; 32],
    /// Stream instance within that connection.
    pub stream: u32,
}

/// Per-call controls supplied by trusted local composition.
pub struct Context<'a> {
    /// Supported local contract version.
    pub version: u32,
    /// Owner-issued action and identity scoped authority.
    pub grant: &'a Grant,
    /// Absolute monotonic deadline, not a peer clock.
    pub deadline: Instant,
    /// Adapter-observed cancellation before effects.
    pub cancelled: bool,
}
impl Context<'_> {
    /// Validate version, call lifetime and receiving owner's authority before effects.
    /// A timeout after possible commitment must instead resolve through durable status.
    /// # Errors
    /// Unsupported versions, Cancelled calls, Denied grants and Limit overruns fail.
    pub fn check(
        &self,
        authority: &Authority,
        local_identity: IdentityId,
        action: Action,
        bytes: usize,
    ) -> Result<(), Error> {
        tracing::trace!("Checking Ledger call version, lifetime and authority");
        if self.version != CONTRACT_VERSION {
            return Err(Error::Unsupported);
        }
        if self.cancelled || Instant::now() >= self.deadline {
            return Err(Error::Cancelled);
        }
        authority.check(self.grant, local_identity, action, bytes)
    }
}

/// Finite admission budgets; lifetime replay records cannot be evicted to admit work.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Maximum distinct accounts in each issuer/owner view.
    pub accounts: usize,
    /// Maximum retained durable operation bindings.
    pub operations: usize,
    /// Maximum retained transfer replay records.
    pub transfers: usize,
    /// Maximum retained claim records including terminal states.
    pub claims: usize,
    /// Maximum simultaneous unaccepted transfer quotes.
    pub quotes: usize,
    /// Maximum serialized local record bytes.
    pub state_bytes: usize,
}
impl Limits {
    /// Require positive limits and arithmetic-safe record framing before allocation.
    /// # Errors
    /// Invalid is returned for zero budgets or unrepresentable length overhead.
    pub fn validate(self) -> Result<(), Error> {
        tracing::trace!("Validating finite Ledger admission limits");
        if [
            self.accounts,
            self.operations,
            self.transfers,
            self.claims,
            self.quotes,
            self.state_bytes,
        ]
        .contains(&0)
            || self.state_bytes.checked_add(64).is_none()
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    // Version, cancellation and deadlines fail before protected work; exact grant bounds pass.
    #[test]
    fn context_checks() {
        let a = Authority::new();
        let id = IdentityId([1; 32]);
        let g = a.grant(id, Action::Coordinate, 8).unwrap();
        let mut c = Context {
            version: 1,
            grant: &g,
            deadline: Instant::now() + Duration::from_secs(30),
            cancelled: false,
        };
        assert_eq!(c.check(&a, id, Action::Coordinate, 8), Ok(()));
        assert_eq!(c.check(&a, id, Action::Coordinate, 9), Err(Error::Limit));
        c.version = 2;
        assert_eq!(
            c.check(&a, id, Action::Coordinate, 8),
            Err(Error::Unsupported)
        );
        c.version = 1;
        c.cancelled = true;
        assert_eq!(
            c.check(&a, id, Action::Coordinate, 8),
            Err(Error::Cancelled)
        );
        c.cancelled = false;
        c.deadline = Instant::now();
        assert_eq!(
            c.check(&a, id, Action::Coordinate, 8),
            Err(Error::Cancelled)
        );
    }
    // Every independently configured capacity is positive and framing arithmetic cannot overflow.
    #[test]
    fn limits_are_finite() {
        let l = Limits {
            accounts: 1,
            operations: 1,
            transfers: 1,
            claims: 1,
            quotes: 1,
            state_bytes: 1,
        };
        assert_eq!(l.validate(), Ok(()));
        for field in 0..6 {
            let mut x = l;
            match field {
                0 => x.accounts = 0,
                1 => x.operations = 0,
                2 => x.transfers = 0,
                3 => x.claims = 0,
                4 => x.quotes = 0,
                _ => x.state_bytes = 0,
            };
            assert_eq!(x.validate(), Err(Error::Invalid));
        }
        assert_eq!(
            Limits {
                state_bytes: usize::MAX,
                ..l
            }
            .validate(),
            Err(Error::Invalid)
        );
    }
    // Reconnecting or using another stream/requester creates a distinct accepted attempt.
    #[test]
    fn keys_identify_attempts() {
        let k = OperationKey {
            requester: IdentityId([1; 32]),
            connection: [2; 32],
            stream: 1,
        };
        assert_ne!(k, OperationKey { stream: 2, ..k });
        assert_ne!(
            k,
            OperationKey {
                connection: [3; 32],
                ..k
            }
        );
        assert_ne!(
            k,
            OperationKey {
                requester: IdentityId([4; 32]),
                ..k
            }
        );
    }
}
