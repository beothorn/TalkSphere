//! Finite two-node custom work exercises explicit metadata selection, consent and real two-sided financial commitment.
use super::{
    failure,
    node::{self, Node},
    owners,
};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use talksphere::{
    composition::server::pump::Signal,
    modules::{
        ledger,
        peer_protocol::engine::endpoint::{
            cache::PeerCache,
            requests::{Ceiling, Invocation, Plan, Reply},
        },
        peer_protocol::requester::Spending,
        services::value::Value,
    },
};
struct Directory(PathBuf);
impl Directory {
    // Atomically create one owner-only random root; no operator storage or developer paths are used.
    fn new() -> Result<Self, String> {
        use std::os::unix::fs::DirBuilderExt;
        tracing::trace!("Creating a private ephemeral demonstration root");
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).map_err(failure)?;
        let suffix = nonce.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let path = std::env::temp_dir().join(format!("talksphere-peer-example-{suffix}"));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .map_err(failure)?;
        Ok(Self(path))
    }
}
impl Drop for Directory {
    // Remove only this example's fresh temporary root after all protected node owners have dropped.
    fn drop(&mut self) {
        tracing::trace!("Removing the explicitly temporary demonstration stores");
        if std::fs::remove_dir_all(&self.0).is_err() {
            tracing::warn!(
                "Temporary demonstration cleanup failed; remove its private directory locally"
            );
        }
    }
}
// Real authenticated nodes complete one separately authorized custom operation; no standard family, persistent service or automatic retry is implied.
pub(super) fn run() -> Result<(u64, u64), String> {
    tracing::trace!("Exercising two independently owned authenticated nodes");
    let root = Directory::new()?;
    let ap = root.0.join("requester");
    let bp = root.0.join("provider");
    let aid = node::identity(&ap)?;
    let bid = node::identity(&bp)?;
    let a_owner = owners::compose(aid, bid, true)?;
    let b_owner = owners::compose(bid, aid, false)?;
    let signal = Signal::default();
    let mut a = Node::new(&ap, a_owner.endpoint.clone(), signal.clone())?;
    let mut b = Node::new(&bp, b_owner.endpoint.clone(), signal)?;
    let connection = a.connect(b.address, bid)?;
    node::until(&mut [&mut a, &mut b], |nodes| {
        nodes.iter().all(|n| n.authenticated == 1)
    })?;
    let cache =
        Arc::new(PeerCache::new(a_owner.registry.clone(), Default::default()).map_err(failure)?);
    for plan in [
        Plan::Board(cache.clone()),
        Plan::Definition {
            cache: cache.clone(),
            offering: "echo".into(),
        },
    ] {
        let result = a.request(&a_owner.endpoint, &connection, plan)?;
        node::complete(&mut [&mut a, &mut b], &result)?
            .result
            .map_err(failure)?;
    }
    let selection = cache
        .selection(bid, "echo", "org.example.echo", 1)
        .map_err(failure)?;
    let result = a.request(
        &a_owner.endpoint,
        &connection,
        Plan::Invoke(Box::new(Invocation {
            selection,
            cache,
            input: Value::String("Hello over authenticated CR2SE.".into()),
            spending: Spending {
                maximum: 7,
                payment: Default::default(),
            },
            consent: Arc::new(Ceiling(7)),
        })),
    )?;
    let reply = node::complete(&mut [&mut a, &mut b], &result)?;
    if !matches!(reply.result, Ok(Reply::Work(_))) {
        return Err(failure(reply.result.err()));
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    let ar = a_owner
        .authority
        .grant(aid, ledger::authority::Action::Read, 1_000_000)
        .map_err(failure)?;
    let br = b_owner
        .authority
        .grant(bid, ledger::authority::Action::Read, 1_000_000)
        .map_err(failure)?;
    let av = a_owner
        .ledger
        .view(
            &ledger::contract::Context {
                version: 1,
                grant: &ar,
                deadline,
                cancelled: false,
            },
            bid,
        )
        .map_err(failure)?;
    let bv = b_owner
        .ledger
        .view(
            &ledger::contract::Context {
                version: 1,
                grant: &br,
                deadline,
                cancelled: false,
            },
            aid,
        )
        .map_err(failure)?;
    Ok((av.owned, bv.issued))
}
