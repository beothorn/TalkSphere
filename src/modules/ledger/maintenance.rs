//! Bounded maintenance preserves all accounting obligations and lifetime replay guards.
use super::{Error, authority::Action, contract::Context, persistence::Persistence, state::Ledger};
/// Public finite usage observation, with no bearer or result data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    /// Owned account entries retained after harmless zero-entry compaction.
    pub owned: usize,
    /// Issued account entries retained after harmless zero-entry compaction.
    pub issued: usize,
    /// All operation bindings, including terminal recovery history.
    pub operations: usize,
    /// Permanent accepted transfer replay entries.
    pub transfers: usize,
    /// All available and terminal claim entries.
    pub claims: usize,
}
impl<P: Persistence> Ledger<P> {
    /// Compact only zero balance entries while retaining trust, accepted operations and replay history.
    /// No coordinator retention proof exists yet, so all terminal operation bindings are preserved.
    /// # Errors
    /// Denied administration or invalid/uncertain persistence prevents successful maintenance.
    pub fn maintain(&self, context: &Context<'_>) -> Result<Usage, Error> {
        tracing::trace!("Maintaining bounded Ledger without deleting obligations");
        self.change(context, Action::Administer, |s| {
            s.accounts.owned.retain(|_, v| *v != 0);
            s.accounts.issued.retain(|_, v| *v != 0);
            Ok(Usage {
                owned: s.accounts.owned.len(),
                issued: s.accounts.issued.len(),
                operations: s.operations.len(),
                transfers: s.transfers.len(),
                claims: s.claims.len(),
            })
        })
    }
}

#[cfg(test)]
#[path = "tests/maintenance.rs"]
mod tests;
