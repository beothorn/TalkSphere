//! Independent native owners for one node of the explicit hosted pages exercise: its Services, Ledger, Board and (for hosts)
//! hosted publication stores, all under one node directory, built around the node's shared protected identity.
use super::{failure, node::ProtectedIdentity};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use talksphere::{
    composition::messaging::{NodeIdentity, SystemClock, SystemRandom},
    modules::{
        board,
        hosted_publications::{
            ids::PlacementId,
            journal::Journal,
            participant::Participant,
            provider::Provider as Engine,
            recovery::DeferToServices,
            service::{
                client::{Client, Placements},
                placements::StorePlacements,
                provider::ServiceProvider,
                recovery::resume,
            },
            standard::Catalogue,
            store::{Limits, Store},
            terms::Terms,
        },
        identity::{api::IdentityId, text},
        ledger,
        messaging::{ports::FixedLimits, service::release::LedgerIssuer},
        peer_protocol as peer,
        services::{self, value::Budget},
    },
};

pub(super) type Endpoint =
    peer::engine::endpoint::Endpoint<services::linux::FileStore, ledger::linux::FileStore>;
pub(super) type Runtime = services::runtime::Coordinator<services::linux::FileStore>;

/// Value bounds shared by every hosted publication result in the exercise.
pub(super) const BUDGET: Budget = Budget {
    depth: 8,
    nodes: 4096,
    bytes: 65536,
};
/// Credits a provider's ledger grants every requester, and the matching balance each requester holds at that provider.
const DEMO_CREDITS: u64 = 1_000_000;
const OFFERING: &str = "hosted-publication-standard";
/// A fixed share of a placement's payments released on early removal (percent), the demonstration's peer policy.
const RELEASE_PERCENT: u64 = 50;

/// Everything one node owns; the endpoint is the only handle the Peer Protocol needs.
pub(super) struct Owners {
    pub endpoint: Endpoint,
    pub registry: Arc<services::registry::Registry>,
    pub runtime: Arc<Runtime>,
    pub services: Arc<services::authority::Authority>,
    pub account: Arc<ledger::state::Ledger<ledger::linux::FileStore>>,
    pub ledger_authority: Arc<ledger::authority::Authority>,
    pub known: Arc<Known>,
}

/// What a requester knows about its own placements (the content size it stored); a provider can never choose the
/// size it is paid for.
#[derive(Default)]
pub(super) struct Known(pub Mutex<BTreeMap<PlacementId, u64>>);
impl Placements for Known {
    // Only sizes the exercise recorded are known, like a real requester catalog.
    fn content_size(
        &self,
        _: services::contract::OperationKey,
        _: IdentityId,
        id: PlacementId,
    ) -> Result<u64, services::Error> {
        tracing::trace!("Looking up a placement size in the exercise catalog");
        self.0
            .lock()
            .map_err(|_| services::Error::Unavailable)?
            .get(&id)
            .copied()
            .ok_or(services::Error::Denied)
    }
}

/// The pinned offering published by `issuer`, as a requester validates it from that provider's Board.
pub(super) fn pinned_offering(issuer: IdentityId) -> Result<board::offering::Offering, String> {
    tracing::trace!("Reading the pinned hosted publication offering");
    offering(issuer).map(|(_, parsed)| parsed)
}

// The pinned offering published by `issuer`, parsed through the Board so every field is validated.
fn offering(issuer: IdentityId) -> Result<(serde_json::Value, board::offering::Offering), String> {
    tracing::trace!("Building the pinned hosted publication offering");
    let mut value: serde_json::Value = serde_json::from_str(include_str!(
        "../../src/modules/hosted_publications/fixtures/offering.json"
    ))
    .map_err(failure)?;
    value["creditIssuer"] = text::format(issuer).into();
    // The pinned fixture's 100-byte bounds are too small for a demonstration page; the exercise offers larger bounds.
    value["info"]["maximumContentBytes"] = 4096.into();
    value["info"]["maximumRetentionSeconds"] = 600.into();
    value["info"]["maximumTotalRetentionSeconds"] = 1200.into();
    value["info"]["maximumListResults"] = 100.into();
    let raw =
        serde_json::json!({"version":1,"providedServices":[value.clone()],"wantedServices":[]});
    let offering = board::offering::Board::parse(raw.to_string().as_bytes(), Default::default())
        .map_err(failure)?
        .offerings
        .remove(OFFERING)
        .ok_or("Missing hosted publication offering")?;
    Ok((value, offering))
}

/// Open one node's stores under `root` (created here). A host also owns hosted publication state and publishes the offering;
/// requesters publish an empty Board. `others` are the peers whose accounts this node's ledger funds (as provider) or holds
/// (as requester).
pub(super) fn open(
    root: &Path,
    identity: ProtectedIdentity,
    provider: bool,
    others: &[IdentityId],
) -> Result<Owners, String> {
    tracing::trace!(
        "Opening independent native owners for the authenticated hosted pages exercise"
    );
    std::fs::create_dir(root).map_err(failure)?;
    let signer = Arc::new(NodeIdentity(identity));
    let local = talksphere::modules::messaging::ports::Signer::identity(signer.as_ref());
    let deadline = Instant::now() + Duration::from_secs(30);

    // Ledger first: the hosted participant issues release credits through it.
    let la = Arc::new(ledger::authority::Authority::new());
    let grant = |action| la.grant(local, action, 16_777_216).map_err(failure);
    let admin = grant(ledger::authority::Action::Administer)?;
    let read = grant(ledger::authority::Action::Read)?;
    let ledger_coordinate = grant(ledger::authority::Action::Coordinate)?;
    let release_grant = grant(ledger::authority::Action::Coordinate)?;
    let lc = ledger::contract::Context {
        version: 1,
        grant: &admin,
        deadline,
        cancelled: false,
    };
    let disk = ledger::linux::FileStore::open(&root.join("ledger"), Default::default(), true)
        .map_err(failure)?;
    let account = ledger::state::Ledger::initialize(
        local,
        la.clone(),
        disk,
        Default::default(),
        &lc,
        DEMO_CREDITS * 4,
        10_000,
    )
    .map_err(failure)?;
    for other in others {
        let trust = ledger::accounting::Trust::new(1).map_err(failure)?;
        if provider {
            account.grant(&lc, *other, DEMO_CREDITS).map_err(failure)?;
        } else {
            account
                .set_owned(&lc, *other, DEMO_CREDITS)
                .map_err(failure)?;
        }
        account.set_trust(&lc, *other, trust).map_err(failure)?;
    }
    let account = Arc::new(account);

    let (raw_offering, parsed) = offering(local)?;
    let catalogue = Arc::new(Catalogue::new(Default::default()).map_err(failure)?);
    let definition = catalogue.definition(&parsed).map_err(failure)?;
    let known = Arc::new(Known::default());

    let services = Arc::new(services::authority::Authority::new());
    let sgrant = |action| services.grant(local, action, 16_777_216).map_err(failure);
    let register = sgrant(services::authority::Action::Register)?;
    let recover = sgrant(services::authority::Action::Recover)?;
    let coordinate = sgrant(services::authority::Action::Coordinate)?;
    let sc = services::contract::Context {
        version: 1,
        grant: &register,
        deadline,
        cancelled: false,
    };
    let registry = Arc::new(
        services::registry::Registry::new(
            local,
            services.clone(),
            Default::default(),
            catalogue.clone(),
        )
        .map_err(failure)?,
    );

    // The host's hosted publication state: placements and the participant journal live in their own native directories.
    let mut placements: Arc<dyn Placements> = known.clone();
    let service = if provider {
        let store = Arc::new(
            Store::open(
                &root.join("placements"),
                Limits {
                    placements: 32,
                    content_bytes: 100_000,
                },
            )
            .map_err(failure)?,
        );
        // A host prices renewals from its own durable placements, never from a requester's catalog.
        placements = Arc::new(StorePlacements(store.clone()));
        let terms = Terms::parse(&parsed, local).map_err(failure)?;
        let engine = Engine::new(
            terms,
            store,
            Arc::new(SystemClock),
            Arc::new(SystemRandom),
            Arc::new(FixedLimits::default()),
        )
        .with_release_policy(Arc::new(|payments: u64, minimum: u64| {
            (payments * RELEASE_PERCENT / 100).max(minimum)
        }))
        .with_release_issuer(Arc::new(LedgerIssuer::new(
            account.clone(),
            release_grant,
            Duration::from_secs(60),
        )));
        let participant = Arc::new(Participant::new(
            Arc::new(engine),
            Journal::open(&root.join("journal"), 64).map_err(failure)?,
            Arc::new(SystemRandom),
        ));
        participant.recover(&DeferToServices).map_err(failure)?;
        Some(Arc::new(
            ServiceProvider::new(participant, BUDGET, 65536).map_err(failure)?,
        ))
    } else {
        None
    };
    let family =
        Arc::new(Client::new(definition.clone(), &catalogue, placements, BUDGET).map_err(failure)?);
    registry
        .register(
            &sc,
            family,
            service
                .as_ref()
                .map(|s| -> Arc<dyn services::handler::Provider> { s.clone() }),
        )
        .map_err(failure)?;

    let policy = ledger::terms::Policy {
        requester_credits: true,
        third_party: false,
    };
    let accounting = Arc::new(services::accounting::Ledger::new(
        account.clone(),
        ledger_coordinate,
        policy,
    ));
    let runtime = Arc::new(
        Runtime::open(
            services::runtime::Config {
                local,
                authority: services.clone(),
                limits: Default::default(),
            },
            &services::contract::Context {
                grant: &register,
                ..sc
            },
            services::linux::FileStore::open(&root.join("services"), Default::default(), true)
                .map_err(failure)?,
            accounting,
            registry.clone(),
            true,
        )
        .map_err(failure)?,
    );
    // Startup recovery runs after the coordinator opens and before anything is offered.
    if let Some(service) = &service {
        resume(
            &runtime,
            &services::contract::Context {
                grant: &coordinate,
                ..sc
            },
            service.as_ref(),
        )
        .map_err(failure)?;
    }
    let _ = &recover;

    let ba = Arc::new(board::authority::Authority::new());
    let board = Arc::new(
        board::state::Provider::open(
            local,
            ba.clone(),
            Default::default(),
            registry.clone(),
            Box::new(
                board::linux::FileStore::open(&root.join("board"), Default::default())
                    .map_err(failure)?,
            ),
        )
        .map_err(failure)?,
    );
    let publish = ba
        .grant(local, board::authority::Action::Publish, 16_777_216)
        .map_err(failure)?;
    let empty = br#"{"version":1,"providedServices":[],"wantedServices":[]}"#.to_vec();
    let raw = serde_json::to_vec(&serde_json::json!({
        "version":1,"providedServices":[raw_offering],"wantedServices":[]
    }))
    .map_err(failure)?;
    let definitions = if provider {
        BTreeMap::from([(OFFERING.to_string(), definition.document.bytes().to_vec())])
    } else {
        BTreeMap::new()
    };
    // The offering is published only now that the participant has recovered and the handler is registered.
    let prepared = board::prepared::Prepared::new(
        local,
        if provider { &raw } else { &empty },
        &definitions,
        Default::default(),
        registry.as_ref(),
    )
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

    let gate = Arc::new(
        peer::admission::LedgerGate::new(local, account.clone(), read, policy).map_err(failure)?,
    );
    let endpoint = Endpoint::new(peer::engine::endpoint::Config {
        local,
        authority: Arc::new(peer::authority::Authority::new()),
        services: services.clone(),
        board,
        board_authority: ba,
        registry: registry.clone(),
        runtime: runtime.clone(),
        ledger: account.clone(),
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
            state_bytes: 1_048_576,
            ..Default::default()
        },
        service_limits: Default::default(),
    })
    .map_err(failure)?;
    Ok(Owners {
        endpoint,
        registry,
        runtime,
        services,
        account,
        ledger_authority: la,
        known,
    })
}
