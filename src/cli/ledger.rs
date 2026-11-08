//! Explicit read-only inspection of an existing protected Ledger; no implicit initialization.
use crate::modules::identity::api::IdentityId;
use std::path::Path;
// Inspect only public accounting/status observations under a locally owned exclusive store lock.
#[cfg(target_os = "linux")]
pub(super) fn show(path: &Path, local: IdentityId, account: IdentityId) -> Result<String, String> {
    use crate::modules::ledger::{
        Error,
        authority::{Action, Authority},
        contract::{Context, Limits},
        linux::FileStore,
        state::Ledger,
    };
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    tracing::trace!("Inspecting existing protected Ledger accounting without mutation");
    let result = (|| {
        let limits = Limits::default();
        let authority = Arc::new(Authority::new());
        let grant = authority.grant(local, Action::Read, limits.state_bytes)?;
        let context = Context {
            version: 1,
            grant: &grant,
            deadline: Instant::now() + Duration::from_secs(30),
            cancelled: false,
        };
        let ledger = Ledger::open(
            local,
            authority,
            FileStore::open(path, limits, false)?,
            limits,
            &context,
        )?;
        let view = ledger.view(&context, account)?;
        let owned = ledger.available(&context, account, true)?;
        let issued = ledger.available(&context, account, false)?;
        let (issuance, internal) = ledger.capacities(&context)?;
        let summary = ledger.summary(&context)?;
        Ok::<_, Error>(format!(
            "identity={}\naccount={}\nowned_from_account={}\nissued_to_account={}\navailable_owned={}\navailable_issued={}\ntrust_millionths={}\nissuance_available={}\ninternal_available={}\nreserved={}\nprepared={}\ncommitted={}\naborted={}\npending_payments={}\nunknown_payments={}\nunsettled_payments={}\n",
            crate::modules::identity::text::format(local),
            crate::modules::identity::text::format(account),
            view.owned,
            view.issued,
            owned,
            issued,
            view.trust.millionths(),
            issuance,
            internal,
            summary.reserved,
            summary.prepared,
            summary.committed,
            summary.aborted,
            summary.pending_payments,
            summary.unknown_payments,
            summary.unsettled_payments
        ))
    })();
    result.map_err(|e| format!("Ledger inspection failed ({e:?})"))
}
// Unsupported platforms never fall back to an unprotected state file.
#[cfg(not(target_os = "linux"))]
pub(super) fn show(
    _path: &Path,
    _local: IdentityId,
    _account: IdentityId,
) -> Result<String, String> {
    tracing::trace!("Refusing unsupported protected Ledger inspection");
    Err("Ledger inspection is supported only on Linux".into())
}
