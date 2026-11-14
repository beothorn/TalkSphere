//! Real local node exercises use protected Identity, actual Cryptography/TCP and explicit custom test handlers.
use super::*;
use crate::composition::server::{
    self,
    config::{Config as ServerConfig, Options},
    pump::Signal,
    transport::Transport,
};
use crate::modules::{
    cryptography::random::SystemEntropy,
    identity::{self, api::IdentityProvider},
    transport::api::*,
};
use peer::{
    engine::bridge::{Bridge, Config as BridgeConfig},
    test_support::{family::Echo, world::World},
};
use std::{
    path::{Path, PathBuf},
    task::{Context as TaskContext, Poll, Wake, Waker},
};
mod processes;

struct WakeNode(Signal);
impl Wake for WakeNode {
    // Worker and socket readiness share a retained notification source without a polling sleep.
    fn wake(self: Arc<Self>) {
        self.0.notify();
    }
}
struct Node {
    transport: Transport,
    bridge: Bridge,
    signal: Signal,
    waker: Waker,
    address: std::net::SocketAddr,
    authenticated: usize,
}
impl Node {
    // Use the protected preinitialized credential and an explicitly installed actual endpoint factory.
    fn new(path: &Path, factory: Arc<dyn Factory>, signal: Signal) -> Self {
        let config = ServerConfig::new(Options::local(path.into()).unwrap()).unwrap();
        let credentials = server::credentials::load(&config, false).unwrap();
        let mut transport = server::transport::compose(config, credentials).unwrap();
        let bridge = Bridge::register(
            &mut transport.driver,
            BridgeConfig {
                connections: 8,
                exchanges: 8,
                workers: 2,
                input_bytes: 131_072,
                output_bytes: peer::record::CONTROL_MAX + 5,
                memory_bytes: 64_000_000,
                controls_per_second: 128,
                lifetime: Duration::from_secs(10),
            },
            factory,
        )
        .unwrap();
        let address = transport.listen().unwrap().address;
        Self {
            transport,
            bridge,
            waker: Waker::from(Arc::new(WakeNode(signal.clone()))),
            signal,
            address,
            authenticated: 0,
        }
    }
    // Drain a bounded real reactor turn, preserving original authenticated contexts for the bridge.
    fn poll(&mut self) {
        let mut cx = TaskContext::from_waker(&self.waker);
        for _ in 0..64 {
            let Poll::Ready(Some(event)) = self.transport.driver.poll_event(&mut cx) else {
                break;
            };
            self.transport.observe(&event).unwrap();
            if let Some(Event::Authenticated(_)) = self
                .bridge
                .event(&mut self.transport.driver, event)
                .unwrap()
            {
                self.authenticated += 1;
            }
        }
        self.bridge
            .poll(&mut self.transport.driver, &mut cx)
            .unwrap();
    }
    // Pin the expected public identity before authenticating a real outgoing TCP connection.
    fn connect(
        &mut self,
        address: std::net::SocketAddr,
        peer: IdentityId,
    ) -> crate::modules::transport::handles::ConnectionHandle {
        let Accepted::Connection(handle) = self
            .transport
            .driver
            .submit(Request::Connect(
                ConnectRequest::new(
                    address,
                    ExpectedPeer::Known(crate::modules::transport::ports::identity::IdentityId(
                        peer.0,
                    )),
                )
                .unwrap(),
            ))
            .unwrap()
        else {
            panic!("connection")
        };
        handle
    }
    // Open one explicit request factory; completion remains inaccessible until the actual Transport terminal gate.
    fn request(
        &mut self,
        endpoint: &Endpoint<services::persistence::Memory, ledger::persistence::Memory>,
        connection: &crate::modules::transport::handles::ConnectionHandle,
        plan: requests::Plan,
    ) -> std::sync::mpsc::Receiver<requests::Completion> {
        let (call, complete) = endpoint.request(plan);
        self.bridge
            .open_with(&mut self.transport.driver, connection, Arc::new(call))
            .unwrap();
        complete
    }
}
// Initialize a protected Identity once, releasing its exclusive store lock before real Transport loads it.
fn identity(path: &Path) -> IdentityId {
    let limits = identity::config::Limits::new(256, 8, 4096, 8192, 4096).unwrap();
    let (store, admin) = identity::storage::FileStore::open(path, limits).unwrap();
    let mut provider = identity::provider::Identity::new(store, SystemEntropy, limits, 1).unwrap();
    let (_, public) = provider.create(&admin).unwrap();
    public.identity.id()
}
struct Directory(PathBuf);
impl Directory {
    // Atomically create a private randomly named root so parallel instrumented processes cannot collide.
    fn new() -> Self {
        use std::os::unix::fs::DirBuilderExt;
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).unwrap();
        let name = nonce
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let path = std::env::temp_dir().join(format!("talksphere-peer-nodes-{name}"));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    // Remove only this fixture's randomly created root after all node/process owners drop.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
// Actual nodes poll readiness and workers until a bounded predicate completes; no fixed-port or startup sleep assumptions.
fn until(nodes: &mut [&mut Node], mut done: impl FnMut(&[&mut Node]) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        for node in nodes.iter_mut() {
            node.poll();
        }
        if done(nodes) {
            return;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(!remaining.is_zero(), "actual node exchange stalled");
        nodes[0]
            .signal
            .wait(remaining.min(Duration::from_millis(50)))
            .unwrap();
    }
}
// Poll real node(s) until one bounded completion is available; no provisional result is treated as success.
fn complete(
    nodes: &mut [&mut Node],
    result: &std::sync::mpsc::Receiver<requests::Completion>,
) -> requests::Completion {
    let mut completed = None;
    until(nodes, |_| {
        completed = result.try_recv().ok();
        completed.is_some()
    });
    completed.unwrap()
}

#[path = "wanted.rs"]
mod wanted_cases;

mod settlement;
