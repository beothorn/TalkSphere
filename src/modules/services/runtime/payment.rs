//! Payment observations and explicit accounting events after work completion.
use super::*;

impl<P: Persistence> Coordinator<P> {
    /// Observe authoritative payment state only after local service work is committed.
    /// # Errors
    /// Unknown gates incomplete/missing accounting state; caller and dependency failures stay typed.
    pub fn payment(
        &self,
        context: &Context<'_>,
        key: OperationKey,
    ) -> Result<crate::modules::ledger::payment::Payment, Error> {
        tracing::trace!("Observing service-independent issuer payment state");
        let state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Observe, &state)?;
        if state
            .records
            .get(&key)
            .ok_or(Error::Denied)?
            .check
            .as_ref()
            .is_some_and(crate::modules::services::checks::Info::included)
        {
            return Err(Error::Unsupported);
        }
        if state.records.get(&key).ok_or(Error::Denied)?.phase != Phase::Committed {
            return Err(Error::Unknown);
        }
        self.accounting.payment(context, key)?.ok_or(Error::Unknown)
    }
    /// Describe an explicit original issuer transfer/query without performing network or signing work.
    /// # Errors
    /// Requires committed work, coordinator authority and supported third-party payment binding.
    pub fn payment_request(
        &self,
        context: &Context<'_>,
        key: OperationKey,
    ) -> Result<crate::modules::ledger::payment::PaymentRequest, Error> {
        tracing::trace!("Describing explicit issuer payment action for composition");
        let state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Coordinate, &state)?;
        if state
            .records
            .get(&key)
            .ok_or(Error::Denied)?
            .check
            .as_ref()
            .is_some_and(crate::modules::services::checks::Info::included)
        {
            return Err(Error::Unsupported);
        }
        if state.records.get(&key).ok_or(Error::Denied)?.phase != Phase::Committed {
            return Err(Error::Unknown);
        }
        self.accounting.payment_request(context, key)
    }
    /// Confirm payment only from the receiver-bound original-issuer evidence object.
    /// # Errors
    /// Wrong provenance, changed amount/fee/roles or missing commitment fail without executing work again.
    pub fn confirm_payment(
        &self,
        context: &Context<'_>,
        key: OperationKey,
        receipt: &crate::modules::ledger::payment::Receipt,
    ) -> Result<(), Error> {
        tracing::trace!("Confirming original issuer evidence independently of service result");
        let state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Coordinate, &state)?;
        if state
            .records
            .get(&key)
            .ok_or(Error::Denied)?
            .check
            .as_ref()
            .is_some_and(crate::modules::services::checks::Info::included)
        {
            return Err(Error::Unsupported);
        }
        if state.records.get(&key).ok_or(Error::Denied)?.phase != Phase::Committed {
            return Err(Error::Unknown);
        }
        self.accounting.confirm_payment(context, key, receipt)
    }
    /// Retain refused or unknown payment without reversing successful local work or releasing its obligation.
    /// # Errors
    /// Requires committed third-party work, coordinator authority and an unresolved payment.
    pub fn payment_problem(
        &self,
        context: &Context<'_>,
        key: OperationKey,
        unknown: bool,
    ) -> Result<(), Error> {
        tracing::trace!("Recording independent issuer refusal or delivery uncertainty");
        let state = self.state.lock().map_err(|_| Error::Unavailable)?;
        self.check(context, Action::Coordinate, &state)?;
        if state
            .records
            .get(&key)
            .ok_or(Error::Denied)?
            .check
            .as_ref()
            .is_some_and(crate::modules::services::checks::Info::included)
        {
            return Err(Error::Unsupported);
        }
        if state.records.get(&key).ok_or(Error::Denied)?.phase != Phase::Committed {
            return Err(Error::Unknown);
        }
        self.accounting.payment_problem(context, key, unknown)
    }
}
