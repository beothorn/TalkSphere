//! Node administration commands; private keys and object content never become CLI output.
use std::path::Path;

// Create a fresh node only on an explicit request and print its public identity.
#[cfg(target_os = "linux")]
pub(super) fn init(root: &Path, config: Option<&Path>) -> Result<String, String> {
    use crate::composition::node::init;
    use crate::modules::identity::text;
    tracing::trace!("Executing explicit node initialization");
    let document = config.map(init::read_document).transpose()?;
    init::initialize(root, document.as_deref()).map(text::format)
}
// Inspect an existing node root without starting it.
#[cfg(target_os = "linux")]
pub(super) fn show(root: &Path) -> Result<String, String> {
    tracing::trace!("Executing node inspection");
    crate::composition::node::init::describe(root)
}
// Unsupported platforms must not fall back to unprotected storage.
#[cfg(not(target_os = "linux"))]
pub(super) fn init(_root: &Path, _config: Option<&Path>) -> Result<String, String> {
    tracing::trace!("Rejecting unsupported node platform");
    Err("the node is unsupported on this platform".into())
}
// Unsupported platforms must not fall back to unprotected storage.
#[cfg(not(target_os = "linux"))]
pub(super) fn show(_root: &Path) -> Result<String, String> {
    tracing::trace!("Rejecting unsupported node platform");
    Err("the node is unsupported on this platform".into())
}
