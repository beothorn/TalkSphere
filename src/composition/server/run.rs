//! Executable runtime orchestration over real Transport; application handlers remain opt-in local composition.
use super::{
    Failure, Stage,
    pump::{EVENTS_PER_TURN, Pump, Signal},
    routing::Router,
    transport::{Ready, Transport},
};
use crate::modules::transport::api::{Event, PeerContext};
use std::time::{Duration, Instant};
/// Public status observations contain no application payload or private store representation.
#[derive(Debug)]
pub enum Status {
    /// All startup prerequisites and the configured bind succeeded.
    Ready(Ready),
    /// Peer proof and session derivation succeeded for this connection.
    Authenticated(PeerContext),
    /// One-shot configured contact admission failed; listener service continues.
    ContactFailed,
    /// A connection ended, independently of any completed service outcome.
    ConnectionClosed,
    /// Driver and protected resources have been dropped after normal shutdown.
    Stopped,
}
/// Run a real server until injected local shutdown, routing only verified events.
/// # Errors
/// Returns startup/runtime/consumer failures or ShutdownTimeout after dropping owned resources.
/// Status callbacks are trusted local observers and must avoid blocking or retaining unbounded output.
pub fn run(
    mut node: Transport,
    mut router: Router,
    signal: Signal,
    mut report: impl FnMut(Status),
) -> Result<(), Failure> {
    tracing::trace!("Starting composed authenticated server runtime");
    if signal.stopping() {
        drop(node);
        report(Status::Stopped);
        return Ok(());
    }
    let ready = node.listen()?;
    report(Status::Ready(ready));
    for attempt in node.connect_contacts()? {
        if attempt.admitted.is_err() {
            tracing::warn!("Configured contact was not admitted; listener remains available");
            report(Status::ContactFailed);
        }
    }
    let mut pump = Pump::new(node, signal.clone());
    let result = (|| {
        loop {
            if signal.stopping() && pump.transport.stage == Stage::Ready {
                pump.transport.begin_shutdown(Instant::now())?;
            }
            let count = pump.turn_with(|node, event| {
                node.observe(&event)?;
                match &event {
                    Event::Authenticated(context) => report(Status::Authenticated(context.clone())),
                    Event::ConnectionClosed { .. } => report(Status::ConnectionClosed),
                    _ => {}
                }
                let _ = router.route(&mut node.driver, event)?;
                Ok(())
            })?;
            if pump.transport.stage == Stage::Stopping
                && pump
                    .transport
                    .shutdown_progress(Instant::now(), signal.forcing())?
            {
                break;
            }
            if count < EVENTS_PER_TURN {
                let wait = pump
                    .transport
                    .shutdown_deadline
                    .map_or(Duration::from_secs(1), |deadline| {
                        deadline.saturating_duration_since(Instant::now())
                    });
                signal.wait(wait)?;
            }
        }
        Ok(())
    })();
    drop(pump);
    if result.is_ok() {
        tracing::info!("Server stopped and released owned resources");
        report(Status::Stopped);
    }
    result
}
