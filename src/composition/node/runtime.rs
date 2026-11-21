//! The node runtime: one loop owning Transport and the Peer Protocol bridge.
//!
//! Callers: `serve --node-root`. Startup order is fixed: load the protected identity, open
//! and recover every durable owner, publish the Board, register Peer Protocol with the
//! authenticated Transport, then listen. Nothing is reachable from the network before
//! recovery finishes. The loop alone touches Transport; local control workers reach it
//! only through the [`Link`] command queue.
use super::{
    config::NodeConfig,
    layout::Layout,
    link::{Command, Link},
    owners::{self, Owners},
};
use crate::composition::server::{
    Stage,
    config::Config,
    credentials,
    pump::{EVENTS_PER_TURN, Signal},
    run::Status,
    transport::{self, Transport},
};
use crate::modules::{
    identity::api::IdentityId,
    peer_protocol::{
        self as peer,
        engine::bridge::{Bridge, Config as BridgeConfig},
    },
    transport::{
        api::{Accepted, ConnectRequest, Event, ExpectedPeer, Request, TransportControl},
        handles::{ConnectionHandle, ConnectionId},
        ports::identity::IdentityId as WireId,
    },
};
use std::{
    sync::{
        Arc,
        mpsc::{Receiver, SyncSender},
    },
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};

/// Longest lifetime of one peer exchange, measured locally.
const EXCHANGE_LIFETIME: Duration = Duration::from_secs(30);
/// Pause between drain checks, because the signal wait returns at once while stopping.
const DRAIN_POLL: Duration = Duration::from_millis(5);
/// Wait granularity when no event or command is pending.
const IDLE_WAIT: Duration = Duration::from_secs(1);

/// Everything the control channel needs once the node is open.
pub struct Started {
    /// The recovered owners.
    pub owners: Arc<Owners>,
    /// Worker command link into the loop.
    pub link: Link,
    /// Wake and shutdown signal shared with the loop.
    pub signal: Signal,
    /// Actual listening address of this node.
    pub listen: std::net::SocketAddr,
}

/// Run the node until shutdown, calling `report` for lifecycle observations.
/// `control` starts the local control channel once the node is ready and returns a guard
/// that is dropped (stopping control) before shutdown drains peer work.
/// # Errors
/// Any startup stage failure, a listener or runtime failure, or a forced shutdown.
pub fn run<G>(
    layout: &Layout,
    server: Config,
    signal: Signal,
    mut report: impl FnMut(Status),
    control: impl FnOnce(&Started) -> Result<G, String>,
) -> Result<(), String> {
    tracing::trace!("Starting the node runtime");
    let config = read_config(layout)?;
    let credentials =
        credentials::load(&server, false).map_err(|e| format!("identity failed: {e:?}"))?;
    let local = IdentityId(credentials.local_id.0);
    let owners = Arc::new(owners::open(layout, config, local, false)?);
    owners.publish()?;
    let mut transport =
        transport::compose(server, credentials).map_err(|e| format!("transport failed: {e:?}"))?;
    let limits = bridge_config(&transport);
    let mut bridge = Bridge::register(&mut transport, limits, Arc::new(owners.endpoint.clone()))
        .map_err(|e| format!("Peer Protocol registration failed: {e:?}"))?;
    let (link, commands) = Link::new(signal.clone());
    if signal.stopping() {
        report(Status::Stopped);
        return Ok(());
    }
    let ready = transport
        .listen()
        .map_err(|e| format!("listener failed: {e:?}"))?;
    report(Status::Ready(ready));
    for attempt in transport
        .connect_contacts()
        .map_err(|e| format!("contacts failed: {e:?}"))?
    {
        if attempt.admitted.is_err() {
            tracing::warn!("Configured contact was not admitted; the listener remains available");
            report(Status::ContactFailed);
        }
    }
    let guard = control(&Started {
        owners: owners.clone(),
        link: link.clone(),
        signal: signal.clone(),
        listen: ready.address,
    })?;
    let result = drive(
        &mut transport,
        &mut bridge,
        &link,
        &commands,
        &signal,
        &mut report,
    );
    // Stop control first so no new request starts while peer work drains.
    drop(guard);
    drop(commands);
    bridge.stop();
    drop(bridge);
    drop(transport);
    drop(owners);
    if result.is_ok() {
        tracing::info!("Node stopped and released owned resources");
        report(Status::Stopped);
    }
    result
}

// Read the node policy; a missing or invalid document stops startup.
fn read_config(layout: &Layout) -> Result<NodeConfig, String> {
    tracing::trace!("Reading the node configuration for startup");
    let bytes = std::fs::read(layout.config())
        .map_err(|_| "node configuration is unavailable".to_string())?;
    NodeConfig::parse(&bytes).map_err(|e| e.to_string())
}

// Bound the bridge by the server's connection limit; exchanges and workers are fixed finite values.
fn bridge_config(transport: &Transport) -> BridgeConfig {
    tracing::trace!("Deriving the Peer Protocol bridge limits");
    BridgeConfig {
        connections: transport.config.options().transport.limits().connections,
        exchanges: 32,
        workers: 4,
        input_bytes: 131_072,
        output_bytes: peer::record::CONTROL_MAX + 5,
        memory_bytes: 128_000_000,
        controls_per_second: 64,
        lifetime: EXCHANGE_LIFETIME,
    }
}

// Wake the loop for worker completion and socket readiness through the shared signal.
struct Notify(Signal);
impl Wake for Notify {
    // Both wake spellings record the same persistent notification.
    fn wake(self: Arc<Self>) {
        self.0.notify();
    }
}

type Pending = Vec<(
    ConnectionHandle,
    SyncSender<Result<ConnectionHandle, String>>,
)>;

// Poll events, bridge work and commands until the drain completes or is forced.
fn drive(
    transport: &mut Transport,
    bridge: &mut Bridge,
    link: &Link,
    commands: &Receiver<Command>,
    signal: &Signal,
    report: &mut impl FnMut(Status),
) -> Result<(), String> {
    tracing::trace!("Driving the node event loop");
    let waker = Waker::from(Arc::new(Notify(signal.clone())));
    let mut cx = Context::from_waker(&waker);
    let mut pending: Pending = Vec::new();
    loop {
        if signal.stopping() && transport.stage == Stage::Ready {
            transport
                .begin_shutdown(Instant::now())
                .map_err(|e| format!("shutdown failed: {e:?}"))?;
            bridge.stop();
            fail_pending(&mut pending);
        }
        let mut busy = serve_commands(transport, bridge, commands, &mut pending, signal.stopping());
        for _ in 0..EVENTS_PER_TURN {
            let Poll::Ready(Some(event)) = transport.poll_event(&mut cx) else {
                break;
            };
            busy = true;
            transport
                .observe(&event)
                .map_err(|e| format!("transport failed: {e:?}"))?;
            route(transport, bridge, link, &mut pending, event, report);
        }
        bridge
            .poll(transport, &mut cx)
            .map_err(|e| format!("peer exchange failed: {e:?}"))?;
        if transport.stage == Stage::Stopping
            && transport
                .shutdown_progress(Instant::now(), signal.forcing())
                .map_err(|e| format!("shutdown failed: {e:?}"))?
        {
            return Ok(());
        }
        if !busy {
            let wait = transport
                .shutdown_remaining(Instant::now())
                .map_or(IDLE_WAIT, |left| left.min(IDLE_WAIT));
            signal
                .wait(wait)
                .map_err(|e| format!("wait failed: {e:?}"))?;
            if signal.stopping() {
                // Signal::wait does not block while stopping; avoid spinning during the drain.
                std::thread::sleep(DRAIN_POLL);
            }
        }
    }
}

// Answer every queued worker command; returns whether any command was handled.
fn serve_commands(
    transport: &mut Transport,
    bridge: &mut Bridge,
    commands: &Receiver<Command>,
    pending: &mut Pending,
    stopping: bool,
) -> bool {
    tracing::trace!("Serving queued worker commands");
    let mut handled = false;
    while let Ok(command) = commands.try_recv() {
        handled = true;
        match command {
            Command::Connect {
                address,
                peer,
                reply,
            } => {
                if stopping {
                    let _ = reply.send(Err("the node is stopping".into()));
                    continue;
                }
                let request = ConnectRequest::new(address, ExpectedPeer::Known(WireId(peer.0)));
                match request
                    .map_err(|_| "contact is not usable".to_string())
                    .and_then(|request| {
                        transport
                            .submit(Request::Connect(request))
                            .map_err(|_| "connection was refused".to_string())
                    }) {
                    Ok(Accepted::Connection(handle)) => pending.push((handle, reply)),
                    Ok(_) => {
                        let _ = reply.send(Err("unexpected connection admission".into()));
                    }
                    Err(error) => {
                        let _ = reply.send(Err(error));
                    }
                }
            }
            Command::Open {
                connection,
                factory,
                reply,
            } => {
                let result = bridge
                    .open_with(transport, &connection, factory)
                    .map(|_| ())
                    .map_err(|e| format!("exchange was refused: {e:?}"));
                let _ = reply.send(result);
            }
        }
    }
    handled
}

// Route one observation: answer waiting connects, forget closed sessions, hand the rest to the bridge.
fn route(
    transport: &mut Transport,
    bridge: &mut Bridge,
    link: &Link,
    pending: &mut Pending,
    event: Event,
    report: &mut impl FnMut(Status),
) {
    tracing::trace!("Routing one verified observation");
    let authenticated = match &event {
        Event::Authenticated(context) => Some(context.connection.clone()),
        _ => None,
    };
    if let Event::ConnectionClosed { connection, .. } = &event {
        link.closed(connection);
        fail_closed(pending, connection);
        report(Status::ConnectionClosed);
    }
    match bridge.event(transport, event) {
        Ok(Some(Event::Authenticated(context))) => {
            if let Some(index) = pending
                .iter()
                .position(|(h, _)| h.id() == &context.connection)
            {
                let (handle, reply) = pending.remove(index);
                let _ = reply.send(Ok(handle));
            }
            report(Status::Authenticated(context));
        }
        Ok(_) => {}
        Err(error) => {
            tracing::warn!(
                ?error,
                "A connection was refused by the peer adapter and is being closed"
            );
            if let Some(connection) = authenticated {
                close(transport, pending, &connection);
            }
        }
    }
}

// Close the connection whose adapter registration failed and fail any worker waiting on it.
fn close(transport: &mut Transport, pending: &mut Pending, connection: &ConnectionId) {
    tracing::trace!("Closing a connection the peer adapter refused");
    fail_closed(pending, connection);
    let handle = transport
        .connection_handles()
        .find(|h| h.id() == connection)
        .cloned();
    if let Some(handle) = handle {
        let _ = transport.submit(Request::Close { connection: handle });
    }
}

// Fail waiting connects whose connection ended before authentication.
fn fail_closed(pending: &mut Pending, connection: &ConnectionId) {
    tracing::trace!("Failing connects of a closed connection");
    let mut index = 0;
    while index < pending.len() {
        if pending[index].0.id() == connection {
            let (_, reply) = pending.remove(index);
            let _ = reply.send(Err("the peer did not authenticate".into()));
        } else {
            index += 1;
        }
    }
}

// Fail every waiting connect when shutdown begins.
fn fail_pending(pending: &mut Pending) {
    tracing::trace!("Failing waiting connects for shutdown");
    for (_, reply) in pending.drain(..) {
        let _ = reply.send(Err("the node is stopping".into()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composition::{node::init, server::config::Options};
    use std::sync::mpsc;
    fn fresh() -> std::path::PathBuf {
        let mut nonce = [0; 8];
        getrandom::fill(&mut nonce).unwrap();
        std::env::temp_dir().join(format!("node-runtime-{}", u64::from_le_bytes(nonce)))
    }
    fn server(layout: &Layout) -> Config {
        Config::new(Options::local(layout.identity()).unwrap()).unwrap()
    }
    // Start the node in a thread and wait for its Ready observation.
    fn start(
        layout: Layout,
        signal: Signal,
    ) -> (
        std::thread::JoinHandle<Result<(), String>>,
        mpsc::Receiver<String>,
    ) {
        let (events, observed) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            run(
                &layout.clone(),
                server(&layout),
                signal,
                move |status| {
                    let _ = events.send(match status {
                        Status::Ready(_) => "ready".to_string(),
                        Status::Stopped => "stopped".to_string(),
                        other => format!("{other:?}"),
                    });
                },
                |_| Ok(()),
            )
        });
        (thread, observed)
    }
    // An initialized node recovers, publishes, listens, then drains to a clean stop with exit Ok.
    #[test]
    fn node_starts_and_stops_cleanly() {
        let root = fresh();
        init::initialize(&root, None).unwrap();
        let signal = Signal::default();
        let (thread, observed) = start(Layout::new(&root), signal.clone());
        assert_eq!(
            observed.recv_timeout(Duration::from_secs(20)).unwrap(),
            "ready"
        );
        signal.shutdown();
        thread.join().unwrap().unwrap();
        assert_eq!(
            observed.recv_timeout(Duration::from_secs(5)).unwrap(),
            "stopped"
        );
    }
    // A running node holds the exclusive identity, so a second start fails and the first is unaffected.
    #[test]
    fn a_second_node_on_the_same_root_is_refused() {
        let root = fresh();
        init::initialize(&root, None).unwrap();
        let signal = Signal::default();
        let (thread, observed) = start(Layout::new(&root), signal.clone());
        assert_eq!(
            observed.recv_timeout(Duration::from_secs(20)).unwrap(),
            "ready"
        );
        let (second, _) = start(Layout::new(&root), Signal::default());
        assert!(second.join().unwrap().is_err());
        signal.shutdown();
        thread.join().unwrap().unwrap();
    }
    // Missing configuration, an uninitialized root and a pre-requested shutdown never start a listener.
    #[test]
    fn startup_failures_are_explicit() {
        let root = fresh();
        std::fs::create_dir(&root).unwrap();
        let (thread, _) = start(Layout::new(&root), Signal::default());
        assert!(
            thread
                .join()
                .unwrap()
                .unwrap_err()
                .contains("configuration")
        );
        let initialized = fresh();
        init::initialize(&initialized, None).unwrap();
        let signal = Signal::default();
        signal.shutdown();
        let (thread, observed) = start(Layout::new(&initialized), signal);
        thread.join().unwrap().unwrap();
        assert_eq!(
            observed.recv_timeout(Duration::from_secs(5)).unwrap(),
            "stopped"
        );
    }
    // A failing control hook stops startup with its message and releases the node cleanly.
    #[test]
    fn control_startup_failure_stops_the_node() {
        let root = fresh();
        init::initialize(&root, None).unwrap();
        let layout = Layout::new(&root);
        let error = run(
            &layout,
            server(&layout),
            Signal::default(),
            |_| {},
            |_| -> Result<(), String> { Err("control unavailable".into()) },
        )
        .unwrap_err();
        assert_eq!(error, "control unavailable");
        // The identity lock is released, so the node can start again.
        let signal = Signal::default();
        let (thread, observed) = start(layout, signal.clone());
        assert_eq!(
            observed.recv_timeout(Duration::from_secs(20)).unwrap(),
            "ready"
        );
        signal.shutdown();
        thread.join().unwrap().unwrap();
    }
}
