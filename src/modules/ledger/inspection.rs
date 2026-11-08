//! Authorized aggregate status observations omit accepted payloads and claim material.
use super::{
    Error, authority::Action, contract::Context, payment::Payment, persistence::Persistence,
    reservation::Status, state::Ledger,
};
/// Counts of durable participant and unresolved payment states, without operation contents.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Summary {
    /// Reserved operations awaiting preparation.
    pub reserved: usize,
    /// Prepared operations awaiting immutable coordinator decisions.
    pub prepared: usize,
    /// Locally committed operations, including work whose payment is unresolved.
    pub committed: usize,
    /// Definitively aborted operation bindings retained for replay safety.
    pub aborted: usize,
    /// Committed third-party work awaiting issuer confirmation.
    pub pending_payments: usize,
    /// Committed work with unknown issuer outcome.
    pub unknown_payments: usize,
    /// Committed work whose issuer explicitly refused settlement.
    pub unsettled_payments: usize,
}
impl<P: Persistence> Ledger<P> {
    /// Read one consistent aggregate snapshot without exposing result references or bearer material.
    /// # Errors
    /// Requires Read authority and reconciled state; no recovery or mutation is implicit.
    pub fn summary(&self, context: &Context<'_>) -> Result<Summary, Error> {
        tracing::trace!("Reading nonsecret Ledger status summary");
        context.check(&self.authority, self.local, Action::Read, 0)?;
        let i = self.inner.lock().map_err(|_| Error::Unknown)?;
        context.check(&self.authority, self.local, Action::Read, 0)?;
        if i.uncertain {
            return Err(Error::Unknown);
        }
        let mut out = Summary::default();
        for op in i.state.operations.values() {
            match op.status {
                Status::Reserved => out.reserved += 1,
                Status::Prepared => out.prepared += 1,
                Status::Committed => out.committed += 1,
                Status::Aborted => out.aborted += 1,
            }
            if op.status == Status::Committed {
                match op.payment {
                    Payment::Pending => out.pending_payments += 1,
                    Payment::Unknown => out.unknown_payments += 1,
                    Payment::Unsettled => out.unsettled_payments += 1,
                    Payment::Settled => {}
                }
            }
        }
        Ok(out)
    }
}
