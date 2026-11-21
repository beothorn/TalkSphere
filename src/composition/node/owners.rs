//! Opens, creates and recovers every durable owner of one node before it admits peer work.
//!
//! Callers: `node-init` (create) and the node runtime (reopen). Order follows the Storage
//! handoff: Storage bytes and journal, then Ledger and Services under the same identity,
//! then Storage participant recovery against Services decisions, then the Board. Nothing
//! is published or reachable from the network here; publication is a separate step that
//! happens only after every owner has recovered.
use super::{
    adapters::{NativeClock, SystemCrypto},
    catalog::FileCatalog,
    config::NodeConfig,
    layout::Layout,
    offering::{self, Prepared},
};
use crate::modules::{
    board,
    identity::api::IdentityId,
    ledger,
    peer_protocol::{self as peer, engine::endpoint::Endpoint},
    services::{self, value::Budget},
    storage::{self, contract::Clock as _},
};
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};

/// Logical value budget shared by Storage operations; fixes the finite object size profile.
pub const BUDGET: Budget = Budget {
    depth: 12,
    nodes: 4096,
    bytes: 65536,
};
/// Upper bound for each owner call made during startup.
const STARTUP_DEADLINE: Duration = Duration::from_secs(30);
/// Authority lifetime budget for local grants, in bytes of control traffic.
const GRANT_BYTES: usize = 16_777_216;

/// Services coordinator over the native stores.
pub type Runtime = services::runtime::Coordinator<services::linux::FileStore>;
/// Peer Protocol endpoint over the native stores.
pub type NodeEndpoint = Endpoint<services::linux::FileStore, ledger::linux::FileStore>;
/// Native Ledger.
pub type Account = ledger::state::Ledger<ledger::linux::FileStore>;

/// Every recovered owner plus the validated offering, ready for composition.
pub struct Owners {
    /// Public identity of this node.
    pub local: IdentityId,
    /// Validated policy this node was opened with.
    pub config: NodeConfig,
    /// Peer Protocol endpoint for provider handling and requester plans.
    pub endpoint: NodeEndpoint,
    /// Services registry holding the Storage definition and handlers.
    pub registry: Arc<services::registry::Registry>,
    /// Services coordinator with retained original results.
    pub runtime: Arc<Runtime>,
    /// Services authority for local grants.
    pub services: Arc<services::authority::Authority>,
    /// Ledger of issuer-separated balances.
    pub account: Arc<Account>,
    /// Ledger authority for local grants.
    pub ledger_authority: Arc<ledger::authority::Authority>,
    /// Requester lease catalog.
    pub catalog: Arc<FileCatalog>,
    /// Whether a provider handler is installed (an offering is configured).
    pub provider: bool,
    board: Arc<board::state::Provider>,
    board_authority: Arc<board::authority::Authority>,
    prepared: Prepared,
}

/// Open the owners of an existing node, or create fresh ones when `create` is set.
/// Storage participants recover before this returns; nothing is published yet.
/// # Errors
/// Any owner that fails to open or recover stops startup with a message naming the stage.
pub fn open(
    layout: &Layout,
    config: NodeConfig,
    local: IdentityId,
    create: bool,
) -> Result<Owners, String> {
    tracing::trace!("Opening node owners before admitting peer work");
    config.validate().map_err(|e| e.to_string())?;
    let deadline = Instant::now() + STARTUP_DEADLINE;
    let services = Arc::new(services::authority::Authority::new());
    let grant = |action| {
        services
            .grant(local, action, GRANT_BYTES)
            .map_err(stage("Services grant"))
    };
    let register = grant(services::authority::Action::Register)?;
    let recover = grant(services::authority::Action::Recover)?;
    let coordinate = grant(services::authority::Action::Coordinate)?;
    let sc = services::contract::Context {
        version: 1,
        grant: &register,
        deadline,
        cancelled: false,
    };
    let catalogue = Arc::new(
        storage::standard::Catalogue::new(Default::default())
            .map_err(stage("Storage catalogue"))?,
    );
    let registry = Arc::new(
        services::registry::Registry::new(
            local,
            services.clone(),
            Default::default(),
            catalogue.clone(),
        )
        .map_err(stage("Services registry"))?,
    );
    let clock = Arc::new(NativeClock::new()?);
    let prepared = offering::prepare(local, config.offering.as_ref(), &catalogue)?;
    let catalog = Arc::new(
        if create {
            FileCatalog::create(&layout.catalog(), local)
        } else {
            FileCatalog::open(&layout.catalog(), local)
        }
        .map_err(|e| format!("opening the lease catalog failed: {e:?}"))?,
    );
    let storage = if config.offering.is_some() {
        Some(open_storage(layout, &config, local, create, clock.clone())?)
    } else {
        None
    };
    let leases = Arc::new(Expectations {
        local,
        provider: storage
            .as_ref()
            .map(|(_, handler)| -> Arc<dyn storage::requester::client::Leases> { handler.clone() }),
        catalog: catalog.clone(),
    });
    let family = Arc::new(
        storage::requester::client::Client::new(
            prepared.definition.clone(),
            &catalogue,
            leases,
            clock.clone(),
            storage::requester::client::Policy {
                values: BUDGET,
                maximum_clock_difference_seconds: 60,
            },
        )
        .map_err(stage("Storage requester"))?,
    );
    registry
        .register(
            &sc,
            family,
            storage
                .as_ref()
                .map(|(_, handler)| -> Arc<dyn services::handler::Provider> { handler.clone() }),
        )
        .map_err(stage("Storage registration"))?;
    let (ledger_authority, account, read, ledger_coordinate) =
        open_ledger(layout, &config, local, create, deadline)?;
    let policy = ledger::terms::Policy {
        requester_credits: true,
        third_party: config.credits.third_party,
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
            services::linux::FileStore::open(&layout.services(), Default::default(), create)
                .map_err(stage("Services store"))?,
            accounting,
            registry.clone(),
            create,
        )
        .map_err(stage("Services runtime"))?,
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
            clock.now().map_err(stage("Storage clock"))?,
            None,
        )
        .map_err(stage("Storage recovery"))?;
    }
    // The Board holds only derived publication state that every start republishes from the
    // configuration. A withdrawn offering would make the old snapshot unsupported without a
    // provider handler, so drop it and let the empty Board replace it. Storage is untouched.
    if storage.is_none() && !create && layout.board().exists() {
        tracing::debug!("Discarding the stale Board publication of a withdrawn offering");
        std::fs::remove_dir_all(layout.board()).map_err(stage("Board withdrawal"))?;
    }
    let board_authority = Arc::new(board::authority::Authority::new());
    let board = Arc::new(
        board::state::Provider::open(
            local,
            board_authority.clone(),
            Default::default(),
            registry.clone(),
            Box::new(
                board::linux::FileStore::open(&layout.board(), Default::default())
                    .map_err(stage("Board store"))?,
            ),
        )
        .map_err(stage("Board"))?,
    );
    let admission = Arc::new(
        peer::admission::LedgerGate::new(local, account.clone(), read, policy)
            .map_err(stage("Ledger admission"))?,
    );
    let endpoint = Endpoint::new(peer::engine::endpoint::Config {
        local,
        authority: Arc::new(peer::authority::Authority::new()),
        services: services.clone(),
        board: board.clone(),
        board_authority: board_authority.clone(),
        registry: registry.clone(),
        runtime: runtime.clone(),
        ledger: account.clone(),
        ledger_authority: ledger_authority.clone(),
        gate: admission,
        transfer_policy: Arc::new(peer::ledger_control::transfer::Fixed(config.credits.fee)),
        claims: None,
        wanted: None,
        resources: Default::default(),
        limits: peer::contract::Limits {
            exchanges: 8,
            bodies: 16,
            nodes: 4096,
            quotes: 8,
            state_bytes: 1_048_576,
            ..Default::default()
        },
        service_limits: Default::default(),
    })
    .map_err(stage("Peer Protocol endpoint"))?;
    Ok(Owners {
        local,
        provider: storage.is_some(),
        config,
        endpoint,
        registry,
        runtime,
        services,
        account,
        ledger_authority,
        catalog,
        board,
        board_authority,
        prepared,
    })
}

impl Owners {
    /// Publish the configured Board and definition, replacing any earlier publication.
    /// Call only after `open` succeeded, so no offering is visible before recovery.
    /// A requester-only node publishes an empty Board, which withdraws an earlier offering.
    /// # Errors
    /// The Board owner refuses the publication; the previous one stays in place.
    pub fn publish(&self) -> Result<(), String> {
        tracing::trace!("Publishing the node Board after recovery");
        let grant = self
            .board_authority
            .grant(self.local, board::authority::Action::Publish, GRANT_BYTES)
            .map_err(stage("Board grant"))?;
        let definitions = if self.provider {
            BTreeMap::from([(
                offering::OFFERING_ID.to_string(),
                self.prepared.definition_bytes.clone(),
            )])
        } else {
            BTreeMap::new()
        };
        let candidate = board::prepared::Prepared::new(
            self.local,
            &self.prepared.board,
            &definitions,
            Default::default(),
            self.registry.as_ref(),
        )
        .map_err(stage("Board publication"))?;
        self.board
            .publish(
                &board::contract::Context {
                    grant: &grant,
                    deadline: Instant::now() + STARTUP_DEADLINE,
                    cancelled: false,
                },
                candidate,
            )
            .map(|_| ())
            .map_err(stage("Board publish"))
    }
}

/// Original lease expectations for the installed Storage family on a node that is both requester and
/// provider. When the provider is this node, the quote is the provider's own and its lease lives in the
/// provider journal; otherwise the lease was bought elsewhere and lives in the requester catalog.
struct Expectations {
    local: IdentityId,
    provider: Option<Arc<dyn storage::requester::client::Leases>>,
    catalog: Arc<FileCatalog>,
}
impl storage::requester::client::Leases for Expectations {
    // Route by provider so neither role can see the other role's leases.
    fn lease(
        &self,
        key: services::contract::OperationKey,
        provider: IdentityId,
        requester: IdentityId,
        id: [u8; 32],
    ) -> Result<storage::contract::Lease, services::Error> {
        tracing::trace!("Routing original lease expectations by provider role");
        if provider == self.local {
            return self
                .provider
                .as_ref()
                .ok_or(services::Error::Denied)?
                .lease(key, provider, requester, id);
        }
        storage::requester::client::Leases::lease(
            self.catalog.as_ref(),
            key,
            provider,
            requester,
            id,
        )
    }
}

type StorageParts = (
    Arc<storage::recovery::Gate>,
    Arc<storage::provider::Provider>,
);

// Open the durable byte store, journal and gate, and build the provider handler over them.
fn open_storage(
    layout: &Layout,
    config: &NodeConfig,
    local: IdentityId,
    create: bool,
    clock: Arc<NativeClock>,
) -> Result<StorageParts, String> {
    tracing::trace!("Opening the durable Storage owner");
    let capacity = config.capacity;
    let bytes = Arc::new(
        storage::bytes::Store::open(&layout.storage(), create).map_err(stage("Storage bytes"))?,
    );
    let crypto = Arc::new(SystemCrypto);
    let limits = storage::image::Limits {
        bytes: 1_048_576usize
            .saturating_add(4096 * (capacity.leases + capacity.operations.saturating_mul(8)))
            .min(64 * 1_048_576),
        leases: capacity.leases,
        operations: capacity.operations.saturating_mul(8),
        values: BUDGET,
    };
    let journal = if create {
        storage::journal::Journal::initialize(
            local,
            bytes.clone(),
            crypto.clone(),
            limits,
            storage::retention::Anchor::new(clock.now().map_err(stage("Storage clock"))?),
        )
    } else {
        storage::journal::Journal::open(local, bytes.clone(), crypto.clone(), limits)
    }
    .map_err(stage("Storage journal"))?;
    let gate = Arc::new(
        storage::recovery::Gate::new(
            Arc::new(journal),
            bytes,
            storage::contract::Limits {
                bytes: capacity.bytes,
                leases: capacity.leases,
                operations: capacity.operations,
                result_bytes: capacity.result_bytes,
                per_owner: capacity.per_owner,
            },
        )
        .map_err(stage("Storage gate"))?,
    );
    let handler = Arc::new(
        storage::provider::Provider::new(
            gate.clone(),
            crypto,
            clock,
            storage::retrieve_result::Limits {
                result_bytes: 65536,
                object_bytes: super::config::MAXIMUM_OBJECT_BYTES,
                values: BUDGET,
            },
            capacity.leases,
        )
        .map_err(stage("Storage provider"))?,
    );
    Ok((gate, handler))
}

type LedgerParts = (
    Arc<ledger::authority::Authority>,
    Arc<Account>,
    ledger::authority::Grant,
    ledger::authority::Grant,
);

// Open or initialize the Ledger with the configured issuance budgets.
fn open_ledger(
    layout: &Layout,
    config: &NodeConfig,
    local: IdentityId,
    create: bool,
    deadline: Instant,
) -> Result<LedgerParts, String> {
    tracing::trace!("Opening the Ledger owner");
    let authority = Arc::new(ledger::authority::Authority::new());
    let grant = |action| {
        authority
            .grant(local, action, GRANT_BYTES)
            .map_err(stage("Ledger grant"))
    };
    let admin = grant(ledger::authority::Action::Administer)?;
    let read = grant(ledger::authority::Action::Read)?;
    let coordinate = grant(ledger::authority::Action::Coordinate)?;
    let context = ledger::contract::Context {
        version: 1,
        grant: &admin,
        deadline,
        cancelled: false,
    };
    let disk = ledger::linux::FileStore::open(&layout.ledger(), Default::default(), create)
        .map_err(stage("Ledger store"))?;
    let account = if create {
        ledger::state::Ledger::initialize(
            local,
            authority.clone(),
            disk,
            Default::default(),
            &context,
            config.credits.issuance,
            config.credits.internal,
        )
    } else {
        ledger::state::Ledger::open(
            local,
            authority.clone(),
            disk,
            Default::default(),
            &ledger::contract::Context {
                grant: &read,
                ..context
            },
        )
    }
    .map_err(stage("Ledger"))?;
    Ok((authority, Arc::new(account), read, coordinate))
}

// Convert an owner failure into a stage message; owner error enums carry no secrets or payloads.
fn stage<E: std::fmt::Debug>(name: &'static str) -> impl Fn(E) -> String {
    move |error| format!("{name} failed: {error:?}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    const ME: IdentityId = IdentityId([1; 32]);
    // Allocate a fresh private node root for one test.
    fn root() -> Layout {
        use std::os::unix::fs::DirBuilderExt;
        let mut nonce = [0; 8];
        getrandom::fill(&mut nonce).unwrap();
        let path: PathBuf =
            std::env::temp_dir().join(format!("node-owners-{}", u64::from_le_bytes(nonce)));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .unwrap();
        Layout::new(path)
    }
    // A created node reopens with recovery, publishes only afterwards, and publication can repeat.
    #[test]
    fn create_reopen_and_publish_after_recovery() {
        let layout = root();
        let created = open(&layout, NodeConfig::default(), ME, true).unwrap();
        assert!(created.provider);
        drop(created);
        let reopened = open(&layout, NodeConfig::default(), ME, false).unwrap();
        reopened.publish().unwrap();
        reopened.publish().unwrap();
    }
    // A requester-only node installs no provider handler and still publishes (an empty Board).
    #[test]
    fn requester_only_node_has_no_provider() {
        let layout = root();
        let config = NodeConfig::parse(br#"{"version":1,"offering":null}"#).unwrap();
        let owners = open(&layout, config.clone(), ME, true).unwrap();
        assert!(!owners.provider);
        owners.publish().unwrap();
        drop(owners);
        let again = open(&layout, config, ME, false).unwrap();
        assert!(!again.provider);
    }
    // Changed terms and a later withdrawal across restart are accepted and republished.
    #[test]
    fn changed_and_withdrawn_terms_survive_restart() {
        let layout = root();
        drop(open(&layout, NodeConfig::default(), ME, true).unwrap());
        let mut changed = NodeConfig::default();
        changed.offering.as_mut().unwrap().check_price = 9;
        open(&layout, changed, ME, false)
            .unwrap()
            .publish()
            .unwrap();
        let withdrawn = NodeConfig::parse(br#"{"version":1,"offering":null}"#).unwrap();
        open(&layout, withdrawn, ME, false)
            .unwrap()
            .publish()
            .unwrap();
    }
    // Reopening an uninitialized root, or initializing twice, fails with a stage message and no panic.
    #[test]
    fn missing_and_existing_state_fail_explicitly() {
        let layout = root();
        let error = open(&layout, NodeConfig::default(), ME, false)
            .err()
            .unwrap();
        assert!(error.contains("failed"), "{error}");
        drop(open(&layout, NodeConfig::default(), ME, true).unwrap());
        assert!(open(&layout, NodeConfig::default(), ME, true).is_err());
    }
    // Invalid terms stop startup before any owner is created.
    #[test]
    fn invalid_terms_stop_before_creating_state() {
        let layout = root();
        let mut bad = NodeConfig::default();
        bad.offering.as_mut().unwrap().pricing = serde_json::Value::Null;
        assert!(open(&layout, bad, ME, true).is_err());
        assert!(!layout.storage().exists());
    }
    // Optional families have no registered provider, so no peer proposal for them can be admitted;
    // Storage is selectable as a provider only when an offering is configured.
    #[test]
    fn only_storage_is_installed() {
        let layout = root();
        let provider = open(&layout, NodeConfig::default(), ME, true).unwrap();
        assert!(provider.registry.select("cr2se.storage", 1, true).is_ok());
        for family in [
            "cr2se.computation",
            "cr2se.messaging",
            "cr2se.discovery",
            "cr2se.files",
            "cr2se.feeds",
            "cr2se.pages",
        ] {
            assert_eq!(
                provider.registry.select(family, 1, true).err(),
                Some(services::Error::Unsupported),
                "{family}"
            );
            assert_eq!(
                provider.registry.select(family, 1, false).err(),
                Some(services::Error::Unsupported),
                "{family}"
            );
        }
        assert_eq!(
            provider.registry.select("cr2se.storage", 2, true).err(),
            Some(services::Error::Unsupported)
        );
        let only = root();
        let requester = open(
            &only,
            NodeConfig::parse(br#"{"version":1,"offering":null}"#).unwrap(),
            ME,
            true,
        )
        .unwrap();
        assert_eq!(
            requester.registry.select("cr2se.storage", 1, true).err(),
            Some(services::Error::Unsupported)
        );
        assert!(requester.registry.select("cr2se.storage", 1, false).is_ok());
    }
    // A missing owner directory stops startup naming that owner instead of continuing partially.
    #[test]
    fn a_missing_owner_stops_startup() {
        let layout = root();
        drop(open(&layout, NodeConfig::default(), ME, true).unwrap());
        std::fs::remove_dir_all(layout.ledger()).unwrap();
        let error = open(&layout, NodeConfig::default(), ME, false)
            .err()
            .unwrap();
        assert!(error.contains("Ledger"), "{error}");
    }
    // A second opener cannot share the exclusive owners while the first is alive.
    #[test]
    fn owners_are_exclusive() {
        let layout = root();
        let first = open(&layout, NodeConfig::default(), ME, true).unwrap();
        assert!(open(&layout, NodeConfig::default(), ME, false).is_err());
        drop(first);
    }
}
