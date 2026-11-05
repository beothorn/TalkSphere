//! Executable server startup; platform and private-key failures remain explicit runtime errors.
use super::server_args::ServerArgs;
// Compose existing credentials, real adapters and process signal forwarding before entering the loop.
#[cfg(target_os = "linux")]
pub(super) fn execute(args: &ServerArgs) -> Result<(), String> {
    use crate::composition::server::{
        credentials,
        pump::Signal,
        routing::Router,
        run::{Status, run},
        shutdown::Signals,
        transport,
    };
    use crate::modules::identity::{api::IdentityId, text};
    tracing::trace!("Starting authenticated server from validated CLI configuration");
    let config = args.config()?;
    let loaded = credentials::load(&config, args.stored_contacts)
        .map_err(|e| format!("server startup failed: {e:?}"))?;
    let node =
        transport::compose(config, loaded).map_err(|e| format!("server startup failed: {e:?}"))?;
    let control = Signal::default();
    let _signals =
        Signals::install(control.clone()).map_err(|e| format!("server startup failed: {e:?}"))?;
    run(node, Router::default(), control, |status| match status {
        Status::Ready(ready) => println!(
            "ready {} {}",
            text::format(IdentityId(ready.local_id.0)),
            ready.address
        ),
        Status::Authenticated(context) => println!(
            "authenticated {} {:?}",
            text::format(IdentityId(context.peer_identity.0)),
            context.role
        ),
        Status::ContactFailed => println!("contact-failed"),
        Status::ConnectionClosed => println!("connection-closed"),
        Status::Stopped => println!("stopped"),
    })
    .map_err(|e| format!("server runtime failed: {e:?}"))
}
// Unsupported platforms never fall back to unauthenticated or unprotected networking.
#[cfg(not(target_os = "linux"))]
pub(super) fn execute(_args: &ServerArgs) -> Result<(), String> {
    tracing::trace!("Rejecting unsupported authenticated server platform");
    Err("server runtime is unsupported on this platform".into())
}
