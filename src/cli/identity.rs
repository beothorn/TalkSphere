//! Explicit local identity administration; private records never become CLI output.
use std::path::Path;

// Create only on an explicit request; public inspection never generates replacement keys.
#[cfg(target_os = "linux")]
pub(super) fn execute(path: &Path, create: bool) -> Result<String, String> {
    use crate::modules::{
        cryptography::random::SystemEntropy,
        identity::{
            api::IdentityProvider, config::Limits, provider::Identity, storage::FileStore, text,
        },
    };
    tracing::trace!("Executing explicit local identity administration");
    if !create && !path.is_dir() {
        return Err("identity store is unavailable".into());
    }
    let limits = Limits::new(1024, 64, 65548, 65536, 65536).map_err(|e| e.to_string())?;
    let (store, admin) = FileStore::open(path, limits).map_err(|e| e.to_string())?;
    let mut identity = Identity::new(store, SystemEntropy, limits, 1).map_err(|e| e.to_string())?;
    let (_, public) = if create {
        identity.create(&admin)
    } else {
        identity.load(&admin)
    }
    .map_err(|e| e.to_string())?;
    Ok(text::format(public.identity.id()))
}
// Unsupported platforms must not fall back to an unprotected credential file.
#[cfg(not(target_os = "linux"))]
pub(super) fn execute(_path: &Path, _create: bool) -> Result<String, String> {
    tracing::trace!("Rejecting unsupported identity administration platform");
    Err("protected identity storage is unsupported on this platform".into())
}
