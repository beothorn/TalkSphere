//! Exact issuer-separated account values used by the local Ledger owner.
use super::{
    Error,
    authority::{Action, Authority},
    contract::Context,
};
use crate::modules::identity::api::IdentityId;
use std::collections::BTreeMap;

/// Local trust in millionths; the exact interval is zero through one million.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Trust(u32);
impl Trust {
    /// Construct an exact local trust value without floating-point rounding.
    /// # Errors
    /// Invalid for values above one million.
    pub fn new(millionths: u32) -> Result<Self, Error> {
        tracing::trace!("Validating exact local trust");
        if millionths > 1_000_000 {
            return Err(Error::Invalid);
        }
        Ok(Self(millionths))
    }
    /// Return exact millionths; zero represents unknown or explicitly refused trust.
    pub fn millionths(self) -> u32 {
        tracing::trace!("Reading exact local trust");
        self.0
    }
}
/// One local identity's independently controlled accounting maps.
#[derive(Clone, Default)]
pub struct Accounts {
    pub(super) owned: BTreeMap<IdentityId, u64>,
    pub(super) issued: BTreeMap<IdentityId, u64>,
    pub(super) trust: BTreeMap<IdentityId, Trust>,
}
/// Immutable issuer-specific accounting observation; currencies are never summed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct View {
    /// Credits this node believes it owns from the selected peer issuer.
    pub owned: u64,
    /// Local credits recognized for the selected peer owner.
    pub issued: u64,
    /// Local trust in this peer; remote balances do not modify it.
    pub trust: Trust,
}
impl Accounts {
    /// Read a privileged immutable local view for one peer without reconciliation.
    /// # Errors
    /// Checks Read authority/lifetime before disclosure; self-balances are invalid.
    pub fn view(
        &self,
        context: &Context<'_>,
        authority: &Authority,
        local: IdentityId,
        peer: IdentityId,
    ) -> Result<View, Error> {
        tracing::trace!("Reading issuer-separated accounting snapshot");
        context.check(authority, local, Action::Read, 0)?;
        if local == peer {
            return Err(Error::Invalid);
        }
        Ok(View {
            owned: *self.owned.get(&peer).unwrap_or(&0),
            issued: *self.issued.get(&peer).unwrap_or(&0),
            trust: *self.trust.get(&peer).unwrap_or(&Trust::default()),
        })
    }
    /// Report only local issuance recognized for the authenticated control caller.
    /// # Errors
    /// Denies absent Control authority and self-balance requests.
    pub fn remote_balance(
        &self,
        context: &Context<'_>,
        authority: &Authority,
        local: IdentityId,
        authenticated: IdentityId,
    ) -> Result<u64, Error> {
        tracing::trace!("Reading caller-scoped issuer balance");
        context.check(authority, local, Action::Control, 0)?;
        if local == authenticated {
            return Err(Error::Invalid);
        }
        Ok(*self.issued.get(&authenticated).unwrap_or(&0))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    // Separate currencies and remote observations preserve exact maxima and never overwrite local views.
    #[test]
    fn issuer_views_are_exact_and_scoped() {
        let [a, b, c, _] = crate::modules::ledger::test_support::identities();
        let authority = Authority::new();
        let grant = authority.grant(a, Action::Read, 1).unwrap();
        let mut context = Context {
            version: 1,
            grant: &grant,
            deadline: Instant::now() + Duration::from_secs(30),
            cancelled: false,
        };
        let mut book = Accounts::default();
        book.owned.insert(b, 100);
        book.owned.insert(c, 40);
        book.issued.insert(b, u64::MAX);
        assert_eq!(book.view(&context, &authority, a, b).unwrap().owned, 100);
        assert_eq!(book.view(&context, &authority, a, c).unwrap().owned, 40);
        assert_eq!(
            book.view(&context, &authority, a, b).unwrap().trust,
            Trust::default()
        );
        assert_eq!(book.view(&context, &authority, a, a), Err(Error::Invalid));
        assert_eq!(
            book.remote_balance(&context, &authority, a, b),
            Err(Error::Denied)
        );
        let control = authority.grant(a, Action::Control, 1).unwrap();
        context.grant = &control;
        assert_eq!(
            book.remote_balance(&context, &authority, a, b),
            Ok(u64::MAX)
        );
        assert_eq!(
            book.remote_balance(&context, &authority, a, a),
            Err(Error::Invalid)
        );
        assert_eq!(book.owned[&b], 100);
        assert!(Trust::new(1_000_001).is_err());
        assert_eq!(Trust::new(1_000_000).unwrap().millionths(), 1_000_000);
    }
}
