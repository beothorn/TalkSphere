//! Demonstration composition supplies independent volatile Board/Services/Ledger owners and explicit custom handler installation.
use super::{failure, family::Echo};
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};
use talksphere::modules::{
    board,
    identity::{api::IdentityId, text},
    ledger, peer_protocol as peer, services,
};
pub(super) type Endpoint =
    peer::engine::endpoint::Endpoint<services::persistence::Memory, ledger::persistence::Memory>;
pub(super) struct Owners {
    pub endpoint: Endpoint,
    pub registry: Arc<services::registry::Registry>,
    pub ledger: Arc<ledger::state::Ledger<ledger::persistence::Memory>>,
    pub authority: Arc<ledger::authority::Authority>,
}
// Each node installs its own custom contract and authority domains; public metadata never installs executable code or grants spending.
pub(super) fn compose(
    local: IdentityId,
    other: IdentityId,
    requester: bool,
) -> Result<Owners, String> {
    tracing::trace!("Composing explicit volatile example module owners");
    let services = Arc::new(services::authority::Authority::new());
    let register = services
        .grant(local, services::authority::Action::Register, 1_000_000)
        .map_err(failure)?;
    let registry = Arc::new(
        services::registry::Registry::new(
            local,
            services.clone(),
            Default::default(),
            Arc::new(services::registry::NoStandards),
        )
        .map_err(failure)?,
    );
    let (family, _, board_bytes) = Echo::new(text::format(local))?;
    let family = Arc::new(family);
    let deadline = Instant::now() + Duration::from_secs(30);
    let sc = services::contract::Context {
        version: 1,
        grant: &register,
        deadline,
        cancelled: false,
    };
    registry
        .register(&sc, family.clone(), Some(family.clone()))
        .map_err(failure)?;
    let ba = Arc::new(board::authority::Authority::new());
    let board = Arc::new(
        board::state::Provider::new(local, ba.clone(), Default::default(), registry.clone())
            .map_err(failure)?,
    );
    use services::handler::Requester;
    let prepared = board::prepared::Prepared::new(
        local,
        &board_bytes,
        &BTreeMap::from([("echo".into(), family.definition().document.bytes().to_vec())]),
        Default::default(),
        registry.as_ref(),
    )
    .map_err(failure)?;
    let publish = ba
        .grant(local, board::authority::Action::Publish, 1_000_000)
        .map_err(failure)?;
    board
        .publish(
            &board::contract::Context {
                grant: &publish,
                deadline,
                cancelled: false,
            },
            prepared,
        )
        .map_err(failure)?;
    let la = Arc::new(ledger::authority::Authority::new());
    let admin = la
        .grant(local, ledger::authority::Action::Administer, 1_000_000)
        .map_err(failure)?;
    let lc = ledger::contract::Context {
        version: 1,
        grant: &admin,
        deadline,
        cancelled: false,
    };
    let ledger = Arc::new(
        ledger::state::Ledger::initialize(
            local,
            la.clone(),
            Default::default(),
            Default::default(),
            &lc,
            1000,
            100,
        )
        .map_err(failure)?,
    );
    ledger.grant(&lc, other, 100).map_err(failure)?;
    ledger
        .set_trust(
            &lc,
            other,
            ledger::accounting::Trust::new(1).map_err(failure)?,
        )
        .map_err(failure)?;
    if requester {
        ledger.set_owned(&lc, other, 100).map_err(failure)?;
    }
    let read = la
        .grant(local, ledger::authority::Action::Read, 1_000_000)
        .map_err(failure)?;
    let gate = Arc::new(
        peer::admission::LedgerGate::new(local, ledger.clone(), read, Default::default())
            .map_err(failure)?,
    );
    let coordinate = la
        .grant(local, ledger::authority::Action::Coordinate, 1_000_000)
        .map_err(failure)?;
    let accounting = Arc::new(services::accounting::Ledger::new(
        ledger.clone(),
        coordinate,
        Default::default(),
    ));
    let runtime = Arc::new(
        services::runtime::Coordinator::open(
            services::runtime::Config {
                local,
                authority: services.clone(),
                limits: Default::default(),
            },
            &sc,
            Default::default(),
            accounting,
            registry.clone(),
            true,
        )
        .map_err(failure)?,
    );
    let endpoint = Endpoint::new(peer::engine::endpoint::Config {
        local,
        authority: Arc::new(peer::authority::Authority::new()),
        services,
        board,
        board_authority: ba,
        registry: registry.clone(),
        runtime,
        ledger: ledger.clone(),
        ledger_authority: la.clone(),
        gate,
        transfer_policy: Arc::new(peer::ledger_control::transfer::Fixed(0)),
        claims: None,
        wanted: None,
        resources: Default::default(),
        limits: peer::contract::Limits {
            exchanges: 4,
            bodies: 16,
            nodes: 4096,
            quotes: 4,
            state_bytes: 65536,
            ..Default::default()
        },
        service_limits: Default::default(),
    })
    .map_err(failure)?;
    Ok(Owners {
        endpoint,
        registry,
        ledger,
        authority: la,
    })
}
