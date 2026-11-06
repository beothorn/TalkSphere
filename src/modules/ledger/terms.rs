//! Explicit immutable settlement terms; application roles are independent of stream opener.
use super::{Error, accounting::Trust};
use crate::modules::identity::api::IdentityId;

/// Agreed issuer relationship for this one local operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Credits issued by the service provider.
    Provider,
    /// Finite credits issued by the requester in exchange for work.
    Requester,
    /// External issuer confirmation is required after successful work.
    ThirdParty,
    /// Same identity records finite internal resource use, without self-balances.
    Internal,
}
/// Complete immutable accounting association copied from an accepted invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Terms {
    /// Authenticated application requester, regardless of TCP opener.
    pub requester: IdentityId,
    /// Application provider responsible for the work.
    pub provider: IdentityId,
    /// Exact credit issuer; never inferred from endpoint.
    pub issuer: IdentityId,
    /// Positive accepted fixed price or metered maximum.
    pub maximum: u64,
    /// Fixed prices must settle exactly at maximum.
    pub fixed: bool,
    /// Exact accepted contract/quote bytes retained for conflict and recovery checks.
    pub accepted: Vec<u8>,
    /// Fresh issuer-mediated payment identifier, absent in two-party/internal modes.
    pub transfer: Option<[u8; 16]>,
    /// Exact preagreed transfer fee, absent outside third-party mode.
    pub fee: Option<u64>,
    /// Explicit total local spending bound including the third-party fee.
    pub spending_bound: u64,
}
/// Trusted provider-side acceptance policy supplied by local composition, never peer JSON.
#[derive(Debug, Clone, Copy, Default)]
pub struct Policy {
    /// Accept finite requester-issued credits.
    pub requester_credits: bool,
    /// Accept third-party refusal/delivery risk after successful work.
    pub third_party: bool,
}
impl Terms {
    /// Validate local role, positive bounds, payment fields and provider trust/policy.
    /// # Errors
    /// Invalid role/shape/overflow, insufficient spending bound or denied provider policy fails.
    pub fn assess(&self, local: IdentityId, trust: Trust, policy: Policy) -> Result<Mode, Error> {
        tracing::trace!("Assessing explicit local Ledger roles and accepted payment policy");
        if (local != self.requester && local != self.provider)
            || self.maximum == 0
            || self.accepted.is_empty()
        {
            return Err(Error::Invalid);
        }
        let mode = if self.requester == self.provider {
            Mode::Internal
        } else if self.issuer == self.provider {
            Mode::Provider
        } else if self.issuer == self.requester {
            Mode::Requester
        } else {
            Mode::ThirdParty
        };
        if mode == Mode::ThirdParty {
            if self.transfer.is_none() || self.fee.is_none() {
                return Err(Error::Invalid);
            }
        } else if self.transfer.is_some() || self.fee.is_some() {
            return Err(Error::Invalid);
        }
        let bound = self
            .maximum
            .checked_add(self.fee.unwrap_or(0))
            .ok_or(Error::Limit)?;
        if bound > self.spending_bound {
            return Err(Error::Insufficient);
        }
        if local == self.provider {
            if trust.millionths() == 0 {
                return Err(Error::Denied);
            }
            if (mode == Mode::Requester && !policy.requester_credits)
                || (mode == Mode::ThirdParty && !policy.third_party)
            {
                return Err(Error::Unsupported);
            }
        }
        Ok(mode)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    // All issuer relationships preserve explicit roles and provider-only trust/policy checks.
    #[test]
    fn mode_and_refusal_matrix() {
        let [a, b, c, u] = super::super::test_support::identities();
        let trust = Trust::new(1).unwrap();
        let policy = Policy {
            requester_credits: true,
            third_party: true,
        };
        let mut t = Terms {
            requester: a,
            provider: b,
            issuer: b,
            maximum: 7,
            fixed: true,
            accepted: vec![1],
            transfer: None,
            fee: None,
            spending_bound: 9,
        };
        assert_eq!(
            t.assess(a, Trust::default(), Policy::default()),
            Ok(Mode::Provider)
        );
        assert_eq!(t.assess(b, Trust::default(), policy), Err(Error::Denied));
        assert_eq!(t.assess(u, trust, policy), Err(Error::Invalid));
        t.issuer = a;
        assert_eq!(
            t.assess(b, trust, Policy::default()),
            Err(Error::Unsupported)
        );
        assert_eq!(t.assess(b, trust, policy), Ok(Mode::Requester));
        t.issuer = c;
        assert_eq!(t.assess(b, trust, policy), Err(Error::Invalid));
        t.transfer = Some([1; 16]);
        t.fee = Some(2);
        assert_eq!(t.assess(b, trust, policy), Ok(Mode::ThirdParty));
        assert_eq!(
            t.assess(b, trust, Policy::default()),
            Err(Error::Unsupported)
        );
        t.spending_bound = 8;
        assert_eq!(t.assess(a, trust, policy), Err(Error::Insufficient));
        t.maximum = u64::MAX;
        assert_eq!(t.assess(a, trust, policy), Err(Error::Limit));
        t.requester = b;
        t.provider = b;
        t.transfer = None;
        t.fee = None;
        t.maximum = 7;
        t.spending_bound = 7;
        assert_eq!(t.assess(b, trust, policy), Ok(Mode::Internal));
        t.maximum = 0;
        assert_eq!(t.assess(b, trust, policy), Err(Error::Invalid));
    }
}
