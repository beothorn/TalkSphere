//! Real protected Identity and TCP reactor composition for the explicitly authorized finite demonstration.
use super::{failure, owners::Endpoint};
use std::{
    path::Path,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};
use talksphere::{
    composition::server::{
        self,
        config::{Config, Options},
        pump::Signal,
        transport::Transport,
    },
    modules::{
        cryptography::random::SystemEntropy,
        identity::{
            self,
            api::{IdentityId, IdentityProvider},
        },
        peer_protocol::{self as peer, engine::bridge::Bridge},
        transport::{api::*, handles::ConnectionHandle},
    },
};
pub(super) struct Node {
    transport: Transport,
    bridge: Bridge,
    signal: Signal,
    waker: Waker,
    pub address: std::net::SocketAddr,
    pub authenticated: usize,
}
struct Notify(Signal);
impl Wake for Notify {
    // Retain the same wake source for worker completion and actual OS socket readiness.
    fn wake(self: Arc<Self>) {
        self.0.notify();
    }
}
// Create a fresh protected credential only within this explicitly owned temporary demonstration directory.
pub(super) fn identity(path: &Path) -> Result<IdentityId, String> {
    tracing::trace!("Initializing an explicitly temporary protected example identity");
    let limits = identity::config::Limits::new(256, 8, 4096, 8192, 4096).map_err(failure)?;
    let (store, admin) = identity::storage::FileStore::open(path, limits).map_err(failure)?;
    let mut provider =
        identity::provider::Identity::new(store, SystemEntropy, limits, 1).map_err(failure)?;
    let (_, public) = provider.create(&admin).map_err(failure)?;
    Ok(public.identity.id())
}
impl Node {
    // Load the already initialized protected credential and explicitly register this complete custom-service endpoint.
    pub(super) fn new(path: &Path, endpoint: Endpoint, signal: Signal) -> Result<Self, String> {
        tracing::trace!("Composing an explicit authenticated example node");
        let config = Config::new(Options::local(path.into()).map_err(failure)?).map_err(failure)?;
        let credentials = server::credentials::load(&config, false).map_err(failure)?;
        let mut transport = server::transport::compose(config, credentials).map_err(failure)?;
        let bridge = Bridge::register(
            &mut transport.driver,
            peer::engine::bridge::Config {
                connections: 4,
                exchanges: 4,
                workers: 2,
                input_bytes: 131072,
                output_bytes: peer::record::CONTROL_MAX + 5,
                memory_bytes: 32_000_000,
                controls_per_second: 32,
                lifetime: Duration::from_secs(10),
            },
            Arc::new(endpoint),
        )
        .map_err(failure)?;
        let address = transport.listen().map_err(failure)?.address;
        Ok(Self {
            transport,
            bridge,
            signal: signal.clone(),
            waker: Waker::from(Arc::new(Notify(signal))),
            address,
            authenticated: 0,
        })
    }
    // Pin the expected remote identity before a real authenticated outgoing connection is admitted.
    pub(super) fn connect(
        &mut self,
        address: std::net::SocketAddr,
        peer: IdentityId,
    ) -> Result<ConnectionHandle, String> {
        tracing::trace!("Connecting an example node to its pinned peer");
        match self
            .transport
            .driver
            .submit(Request::Connect(
                ConnectRequest::new(
                    address,
                    ExpectedPeer::Known(
                        talksphere::modules::transport::ports::identity::IdentityId(peer.0),
                    ),
                )
                .map_err(failure)?,
            ))
            .map_err(failure)?
        {
            Accepted::Connection(handle) => Ok(handle),
            _ => Err("Unexpected connection admission".into()),
        }
    }
    // Poll bounded actual Transport events before asynchronous peer work; payloads never enter diagnostics.
    fn poll(&mut self) -> Result<(), String> {
        tracing::trace!("Polling a bounded example node turn");
        let mut cx = Context::from_waker(&self.waker);
        for _ in 0..64 {
            let Poll::Ready(Some(event)) = self.transport.driver.poll_event(&mut cx) else {
                break;
            };
            self.transport.observe(&event).map_err(failure)?;
            if matches!(
                self.bridge
                    .event(&mut self.transport.driver, event)
                    .map_err(failure)?,
                Some(Event::Authenticated(_))
            ) {
                self.authenticated += 1;
            }
        }
        self.bridge
            .poll(&mut self.transport.driver, &mut cx)
            .map_err(failure)
    }
    // Explicit local request plans retain original scope, local consent and the actual normal-completion gate.
    pub(super) fn request(
        &mut self,
        endpoint: &Endpoint,
        connection: &ConnectionHandle,
        plan: peer::engine::endpoint::requests::Plan,
    ) -> Result<std::sync::mpsc::Receiver<peer::engine::endpoint::requests::Completion>, String>
    {
        tracing::trace!("Opening one explicitly authorized example request");
        let (call, completed) = endpoint.request(plan);
        self.bridge
            .open_with(&mut self.transport.driver, connection, Arc::new(call))
            .map_err(failure)?;
        Ok(completed)
    }
}
// Both actual nodes share readiness notifications and a finite absolute demonstration deadline.
pub(super) fn until(
    nodes: &mut [&mut Node],
    mut done: impl FnMut(&[&mut Node]) -> bool,
) -> Result<(), String> {
    tracing::trace!("Driving actual example nodes until a bounded terminal observation");
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        for node in nodes.iter_mut() {
            node.poll()?;
        }
        if done(nodes) {
            return Ok(());
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(
                "Peer Protocol demonstration deadline expired; accepted outcomes may be unknown"
                    .into(),
            );
        }
        nodes[0].signal.wait(remaining).map_err(failure)?;
    }
}
// Publish only one capacity-one completion after the real Transport and owner terminal gates.
pub(super) fn complete(
    nodes: &mut [&mut Node],
    receiver: &std::sync::mpsc::Receiver<peer::engine::endpoint::requests::Completion>,
) -> Result<peer::engine::endpoint::requests::Completion, String> {
    tracing::trace!("Awaiting a complete original-stream example result");
    let mut result = None;
    until(nodes, |_| {
        result = receiver.try_recv().ok();
        result.is_some()
    })?;
    result.ok_or_else(|| "Missing example completion".into())
}
