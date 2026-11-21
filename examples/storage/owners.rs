//! Independent native owners for the explicit Storage exercise.
use super::failure;
use services::{contract::OperationKey, value::Budget};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use storage::contract::{Clock as _, Lease, Time};
use talksphere::modules::{
    board,
    identity::{api::IdentityId, text},
    ledger, peer_protocol as peer, services, storage,
};
pub(super) type Endpoint =
    peer::engine::endpoint::Endpoint<services::linux::FileStore, ledger::linux::FileStore>;
pub(super) type Runtime = services::runtime::Coordinator<services::linux::FileStore>;
pub(super) const BUDGET: Budget = Budget {
    depth: 12,
    nodes: 4096,
    bytes: 65536,
};

pub(super) struct Owners {
    pub endpoint: Endpoint,
    pub registry: Arc<services::registry::Registry>,
    pub runtime: Arc<Runtime>,
    pub services: Arc<services::authority::Authority>,
    pub account: Arc<ledger::state::Ledger<ledger::linux::FileStore>>,
    pub ledger_authority: Arc<ledger::authority::Authority>,
    #[cfg(test)]
    pub peer_authority: Arc<peer::authority::Authority>,
}

pub(super) struct Catalog(pub Mutex<Option<Lease>>);
impl storage::requester::client::Leases for Catalog {
    // Permit only the catalog's actual lease holder and provider. The exercise serializes mutations.
    fn lease(
        &self,
        key: OperationKey,
        provider: IdentityId,
        requester: IdentityId,
        id: [u8; 32],
    ) -> Result<Lease, services::Error> {
        tracing::trace!("Checking the exercise's original provider-scoped lease metadata");
        let lease = self.0.lock().map_err(|_| services::Error::Unavailable)?;
        if key.requester != requester {
            return Err(services::Error::Denied);
        }
        storage::lease::authorize(lease.as_ref(), provider, requester, &id).cloned()
    }
}

struct Crypto;
impl storage::contract::Crypto for Crypto {
    // Obtain unpredictable identifiers from the host's cryptographic entropy source.
    fn lease_id(&self) -> Result<[u8; 32], services::Error> {
        tracing::trace!("Generating an opaque Storage lease identifier");
        let mut id = [0; 32];
        getrandom::fill(&mut id).map_err(|_| services::Error::Unavailable)?;
        Ok(id)
    }

    // Hash the exact logical bytes without changing the payload.
    fn hash(&self, bytes: &[u8]) -> Result<[u8; 32], services::Error> {
        tracing::trace!("Hashing exact Storage object bytes");
        talksphere::modules::cryptography::primitives::hash::sha256(bytes, bytes.len())
            .map_err(|_| services::Error::Limit)
    }
}

struct Clock([u8; 16]);
impl Clock {
    // A boot identifier stays stable across processes and detects the absence of trusted reboot evidence.
    fn new() -> Result<Self, String> {
        tracing::trace!("Preparing the native Storage exercise clock");
        let id = std::fs::read_to_string("/proc/sys/kernel/random/boot_id").map_err(failure)?;
        let digest = talksphere::modules::cryptography::primitives::hash::sha256(id.as_bytes(), 64)
            .map_err(failure)?;
        Ok(Self(digest[..16].try_into().map_err(failure)?))
    }
}
impl storage::contract::Clock for Clock {
    // Read uptime and UTC separately from the same host; uptime remains comparable across process restarts.
    fn now(&self) -> Result<Time, services::Error> {
        tracing::trace!("Sampling native uptime and UTC for Storage retention");
        let uptime =
            std::fs::read_to_string("/proc/uptime").map_err(|_| services::Error::Unavailable)?;
        let elapsed_seconds = uptime
            .split_whitespace()
            .next()
            .and_then(|s| s.split('.').next())
            .and_then(|s| s.parse().ok())
            .ok_or(services::Error::Invalid)?;
        let unix_seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| services::Error::Invalid)?
            .as_secs();
        Ok(Time {
            unix_seconds,
            elapsed_seconds,
            boot: self.0,
        })
    }
}

// Build a complete independently recognized bounded offering with the local identity as issuer.
fn offering(local: IdentityId) -> Result<serde_json::Value, String> {
    tracing::trace!("Building the bounded demonstration Storage offering");
    let mut value: serde_json::Value = serde_json::from_str(include_str!(
        "../../src/modules/storage/fixtures/offering.json"
    ))
    .map_err(failure)?;
    value["creditIssuer"] = text::format(local).into();
    value["info"]["maximumBytes"] = 8192.into();
    value["info"]["maximumRetrieveBytes"] = 8192.into();
    Ok(value)
}

// Explicitly initialize fresh directories or reopen existing owners under new authority domains.
pub(super) fn open(
    root: &Path,
    local: IdentityId,
    other: IdentityId,
    requester: bool,
    create: bool,
    catalog: Arc<Catalog>,
) -> Result<Owners, String> {
    open_with_wanted(root, local, other, requester, create, catalog, None)
}

// Configure optional local wanted input and consent alongside the actual provided handler.
pub(super) fn open_with_wanted(
    root: &Path,
    local: IdentityId,
    other: IdentityId,
    requester: bool,
    create: bool,
    catalog: Arc<Catalog>,
    wanted: Option<peer::engine::endpoint::wanted::Publisher>,
) -> Result<Owners, String> {
    open_configured(
        root,
        local,
        other,
        requester,
        create,
        catalog,
        PaymentProfile::default(),
        wanted,
    )
}

/// Explicit economic choices for controlled local integration scenarios.
#[derive(Default)]
pub(super) struct PaymentProfile {
    pub issuer: Option<IdentityId>,
    pub fee: u64,
    pub allow_third_party: bool,
}

// Wire an explicit issuer policy without allowing an incoming request to select local policy.
#[allow(clippy::too_many_arguments)]
pub(super) fn open_configured(
    root: &Path,
    local: IdentityId,
    other: IdentityId,
    requester: bool,
    create: bool,
    catalog: Arc<Catalog>,
    payment: PaymentProfile,
    wanted: Option<peer::engine::endpoint::wanted::Publisher>,
) -> Result<Owners, String> {
    tracing::trace!("Opening independent native owners for the authenticated Storage exercise");
    if create {
        std::fs::create_dir(root).map_err(failure)?;
    }
    let deadline = Instant::now() + Duration::from_secs(30);
    let services = Arc::new(services::authority::Authority::new());
    let grant = |action| services.grant(local, action, 16_777_216).map_err(failure);
    let register = grant(services::authority::Action::Register)?;
    let recover = grant(services::authority::Action::Recover)?;
    let coordinate = grant(services::authority::Action::Coordinate)?;
    let sc = services::contract::Context {
        version: 1,
        grant: &register,
        deadline,
        cancelled: false,
    };
    let catalogue =
        Arc::new(storage::standard::Catalogue::new(Default::default()).map_err(failure)?);
    let registry = Arc::new(
        services::registry::Registry::new(
            local,
            services.clone(),
            Default::default(),
            catalogue.clone(),
        )
        .map_err(failure)?,
    );
    let clock = Arc::new(Clock::new()?);
    let offered = offering(payment.issuer.unwrap_or(local))?;
    let wanted_offerings = if wanted.is_some() {
        let mut offer = offered.clone();
        offer["id"] = "storage-wanted".into();
        vec![offer]
    } else {
        vec![]
    };
    let raw = serde_json::to_vec(&serde_json::json!({
        "version":1, "providedServices":[offered], "wantedServices":wanted_offerings
    }))
    .map_err(failure)?;
    let parsed = board::offering::Board::parse(&raw, Default::default()).map_err(failure)?;
    let offering = parsed
        .offerings
        .get("storage-standard")
        .ok_or("Missing Storage offering")?;
    let definition = catalogue.definition(offering).map_err(failure)?;
    let definition_bytes = definition.document.bytes().to_vec();
    let storage = if requester {
        None
    } else {
        let bytes =
            Arc::new(storage::bytes::Store::open(&root.join("storage"), create).map_err(failure)?);
        let crypto = Arc::new(Crypto);
        let limits = storage::image::Limits {
            bytes: 1_048_576,
            leases: 8,
            operations: 32,
            values: BUDGET,
        };
        let journal = if create {
            storage::journal::Journal::initialize(
                local,
                bytes.clone(),
                crypto.clone(),
                limits,
                storage::retention::Anchor::new(clock.now().map_err(failure)?),
            )
        } else {
            storage::journal::Journal::open(local, bytes.clone(), crypto.clone(), limits)
        }
        .map_err(failure)?;
        let gate = Arc::new(
            storage::recovery::Gate::new(
                Arc::new(journal),
                bytes,
                storage::contract::Limits {
                    bytes: 32768,
                    leases: 8,
                    operations: 8,
                    result_bytes: 262144,
                    per_owner: 4,
                },
            )
            .map_err(failure)?,
        );
        let handler = Arc::new(
            storage::provider::Provider::new(
                gate.clone(),
                crypto,
                clock.clone(),
                storage::retrieve_result::Limits {
                    result_bytes: 65536,
                    object_bytes: 8192,
                    values: BUDGET,
                },
                8,
            )
            .map_err(failure)?,
        );
        Some((gate, handler))
    };
    let leases: Arc<dyn storage::requester::client::Leases> = if let Some((_, handler)) = &storage {
        handler.clone()
    } else {
        catalog
    };
    let family = Arc::new(
        storage::requester::client::Client::new(
            definition,
            &catalogue,
            leases,
            clock.clone(),
            storage::requester::client::Policy {
                values: BUDGET,
                maximum_clock_difference_seconds: 60,
            },
        )
        .map_err(failure)?,
    );
    registry
        .register(
            &sc,
            family,
            storage
                .as_ref()
                .map(|(_, handler)| -> Arc<dyn services::handler::Provider> { handler.clone() }),
        )
        .map_err(failure)?;

    let la = Arc::new(ledger::authority::Authority::new());
    let grant = |action| la.grant(local, action, 16_777_216).map_err(failure);
    let admin = grant(ledger::authority::Action::Administer)?;
    let read = grant(ledger::authority::Action::Read)?;
    let ledger_coordinate = grant(ledger::authority::Action::Coordinate)?;
    let lc = ledger::contract::Context {
        version: 1,
        grant: &admin,
        deadline,
        cancelled: false,
    };
    let disk = ledger::linux::FileStore::open(&root.join("ledger"), Default::default(), create)
        .map_err(failure)?;
    let account = if create {
        let account = ledger::state::Ledger::initialize(
            local,
            la.clone(),
            disk,
            Default::default(),
            &lc,
            1_000_000,
            10_000,
        )
        .map_err(failure)?;
        if wanted.is_none() {
            account.grant(&lc, other, 1_000_000).map_err(failure)?;
        }
        account
            .set_trust(
                &lc,
                other,
                ledger::accounting::Trust::new(1).map_err(failure)?,
            )
            .map_err(failure)?;
        if requester {
            account.set_owned(&lc, other, 1_000_000).map_err(failure)?;
        }
        account
    } else {
        ledger::state::Ledger::open(
            local,
            la.clone(),
            disk,
            Default::default(),
            &ledger::contract::Context { grant: &read, ..lc },
        )
        .map_err(failure)?
    };
    let account = Arc::new(account);
    let policy = ledger::terms::Policy {
        requester_credits: true,
        third_party: payment.allow_third_party,
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
                grant: if create { &register } else { &recover },
                ..sc
            },
            services::linux::FileStore::open(&root.join("services"), Default::default(), create)
                .map_err(failure)?,
            accounting,
            registry.clone(),
            create,
        )
        .map_err(failure)?,
    );
    if let Some((gate, _)) = &storage {
        gate.recover(
            &storage::recovery::Services {
                coordinator: &runtime,
                context: &services::contract::Context {
                    grant: &coordinate,
                    ..sc
                },
            },
            clock.now().map_err(failure)?,
            None,
        )
        .map_err(failure)?;
    }
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
    let empty = br#"{"version":1,"providedServices":[],"wantedServices":[]}"#;
    let mut definitions = if requester {
        BTreeMap::new()
    } else {
        BTreeMap::from([("storage-standard".into(), definition_bytes)])
    };
    if wanted.is_some() {
        let offering = parsed
            .offerings
            .get("storage-wanted")
            .ok_or("Missing wanted offering")?;
        let standards = storage::standard::Catalogue::new(Default::default()).map_err(failure)?;
        definitions.insert(
            offering.id.clone(),
            standards
                .definition(offering)
                .map_err(failure)?
                .document
                .bytes()
                .to_vec(),
        );
    }
    let prepared = board::prepared::Prepared::new(
        local,
        if requester { empty } else { &raw },
        &definitions,
        Default::default(),
        registry.as_ref(),
    )
    .map_err(failure)?;
    if create {
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
    }
    let gate = Arc::new(
        peer::admission::LedgerGate::new(local, account.clone(), read, policy).map_err(failure)?,
    );
    let peer_authority = Arc::new(peer::authority::Authority::new());
    let endpoint = Endpoint::new(peer::engine::endpoint::Config {
        local,
        authority: peer_authority.clone(),
        services: services.clone(),
        board,
        board_authority: ba,
        registry: registry.clone(),
        runtime: runtime.clone(),
        ledger: account.clone(),
        ledger_authority: la.clone(),
        gate,
        transfer_policy: Arc::new(peer::ledger_control::transfer::Fixed(payment.fee)),
        claims: None,
        wanted,
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
        #[cfg(test)]
        peer_authority,
    })
}
