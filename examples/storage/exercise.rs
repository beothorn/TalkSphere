//! Authenticated paid Storage with independent durable owners and a provider reopen between store and retrieve.
#[cfg(test)]
#[path = "replicas.rs"]
mod replicas;
#[cfg(test)]
#[path = "wanted.rs"]
mod wanted;
use super::{
    failure,
    node::{self, Node},
    owners::{self, Catalog, Owners},
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use talksphere::{
    composition::server::pump::Signal,
    modules::{
        identity::api::IdentityId,
        peer_protocol::{
            self as peer,
            engine::endpoint::{
                cache::PeerCache,
                requests::{Ceiling, Challenge, Invocation, Plan, Reply},
            },
        },
        services::{self, contract::OperationKey, value::Value},
        storage::contract::{Lease, State},
        transport::handles::ConnectionHandle,
    },
};

struct Directory(PathBuf);
impl Directory {
    // Use atomically allocated owner-only directories, with cleanup scoped to this demonstration.
    fn new() -> Result<Self, String> {
        use std::os::unix::fs::DirBuilderExt;
        tracing::trace!("Creating an independent private Storage exercise root");
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).map_err(failure)?;
        let suffix = nonce.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let root = std::env::temp_dir().join(format!("talksphere-storage-{suffix}"));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .map_err(failure)?;
        Ok(Self(root))
    }
}
impl Drop for Directory {
    // Reap only the temporary root this process created.
    fn drop(&mut self) {
        tracing::trace!("Cleaning up the isolated Storage exercise");
        if std::fs::remove_dir_all(&self.0).is_err() {
            tracing::warn!("The temporary Storage directory could not be removed");
        }
    }
}

enum Host {
    Local(Box<Node>),
    #[cfg(test)]
    Child(super::process::Process),
}

impl Host {
    // Compose one independent provider and keep the transport lifecycle separate from its journals.
    fn start(
        root: &std::path::Path,
        identity: &std::path::Path,
        local: IdentityId,
        requester: IdentityId,
        create: bool,
        signal: Signal,
        separate: bool,
    ) -> Result<Self, String> {
        tracing::trace!("Opening the Storage exercise provider");
        #[cfg(test)]
        if separate {
            return super::process::Process::start(root, identity, local, requester, create)
                .map(Self::Child);
        }
        let _ = separate;
        let owner = owners::open(
            root,
            local,
            requester,
            false,
            create,
            Arc::new(Catalog(Mutex::new(None))),
        )?;
        Ok(Self::Local(Box::new(Node::new(
            identity,
            owner.endpoint,
            signal,
        )?)))
    }

    // Read the actual listening endpoint supplied by this host.
    fn address(&self) -> std::net::SocketAddr {
        tracing::trace!("Reading the original provider listener");
        match self {
            Self::Local(node) => node.address,
            #[cfg(test)]
            Self::Child(child) => child.address,
        }
    }

    // Poll the in-process provider with the requester, or let a subprocess drive its own reactor.
    fn complete(
        &mut self,
        a: &mut Node,
        pending: &std::sync::mpsc::Receiver<peer::engine::endpoint::requests::Completion>,
    ) -> Result<peer::engine::endpoint::requests::Completion, String> {
        tracing::trace!("Waiting for the normally ended Storage request");
        match self {
            Self::Local(b) => node::complete(&mut [a, b], pending),
            #[cfg(test)]
            Self::Child(_) => node::complete(&mut [a], pending),
        }
    }

    // Stop the original provider before reopening its exclusive stores.
    fn stop(self) -> Result<(), String> {
        tracing::trace!("Stopping the original Storage provider");
        match self {
            Self::Local(_) => Ok(()),
            #[cfg(test)]
            Self::Child(mut child) => child.stop(),
        }
    }
}

// Drive one real authenticated connection and fetch only its directly published Board and definition.
fn connect(
    a: &mut Node,
    b: &mut Host,
    owner: &Owners,
    provider: IdentityId,
) -> Result<(ConnectionHandle, Arc<PeerCache>), String> {
    tracing::trace!("Fetching Storage terms directly from the authenticated provider");
    let target = a.authenticated + 1;
    let connection = a.connect(b.address(), provider)?;
    match b {
        Host::Local(b) => node::until(&mut [a, b], |nodes| {
            nodes[0].authenticated == target && nodes[1].authenticated == 1
        })?,
        #[cfg(test)]
        Host::Child(_) => node::until(&mut [a], |nodes| nodes[0].authenticated == target)?,
    }
    let cache =
        Arc::new(PeerCache::new(owner.registry.clone(), Default::default()).map_err(failure)?);
    for plan in [
        Plan::Board(cache.clone()),
        Plan::Definition {
            cache: cache.clone(),
            offering: "storage-standard".into(),
        },
    ] {
        let pending = a.request(&owner.endpoint, &connection, plan)?;
        b.complete(a, &pending)?.result.map_err(failure)?;
    }
    Ok((connection, cache))
}

struct Network<'a> {
    requester: &'a mut Node,
    provider: &'a mut Host,
    owner: &'a Owners,
    connection: ConnectionHandle,
    cache: Arc<PeerCache>,
    provider_id: IdentityId,
    requester_id: IdentityId,
}
impl Network<'_> {
    // Only locally selected operation input and an exact explicit price ceiling can reserve work.
    fn invoke(&mut self, input: Value, maximum: u64) -> Result<OperationKey, String> {
        tracing::trace!("Invoking one authorized Storage operation");
        let selection = self
            .cache
            .selection(self.provider_id, "storage-standard", "cr2se.storage", 1)
            .map_err(failure)?;
        let plan = Plan::Invoke(Box::new(Invocation {
            selection,
            cache: self.cache.clone(),
            input,
            spending: peer::requester::Spending {
                maximum,
                payment: Default::default(),
            },
            consent: Arc::new(Ceiling(maximum)),
        }));
        let pending = self
            .requester
            .request(&self.owner.endpoint, &self.connection, plan)?;
        match self
            .provider
            .complete(self.requester, &pending)?
            .result
            .map_err(failure)?
        {
            Reply::Work(key) => Ok(key),
            _ => Err("Expected an original Storage work result".into()),
        }
    }

    // Compare the independent requester account and actual original issuer balance over the authenticated connection.
    fn balances(&mut self) -> Result<(u64, u64), String> {
        tracing::trace!("Observing both authoritative demo credit balances");
        use talksphere::modules::ledger;
        let grant = self
            .owner
            .ledger_authority
            .grant(
                self.requester_id,
                ledger::authority::Action::Read,
                16_777_216,
            )
            .map_err(failure)?;
        let context = ledger::contract::Context {
            version: 1,
            grant: &grant,
            deadline: Instant::now() + Duration::from_secs(10),
            cancelled: false,
        };
        let local = self
            .owner
            .account
            .view(&context, self.provider_id)
            .map_err(failure)?
            .owned;
        let pending =
            self.requester
                .request(&self.owner.endpoint, &self.connection, Plan::Balance)?;
        let Reply::Ledger(peer::ledger_control::query::Response::Balance(remote)) = self
            .provider
            .complete(self.requester, &pending)?
            .result
            .map_err(failure)?
        else {
            return Err("Expected an original issuer balance observation".into());
        };
        Ok((local, remote))
    }

    // Observe exact retained local result under a fresh observation grant.
    fn result(&self, key: OperationKey) -> Result<services::handler::Completion, String> {
        tracing::trace!("Reading original committed Storage result from requester Services");
        let grant = self
            .owner
            .services
            .grant(
                self.requester_id,
                services::authority::Action::Observe,
                16_777_216,
            )
            .map_err(failure)?;
        let context = services::contract::Context {
            version: 1,
            grant: &grant,
            deadline: Instant::now() + Duration::from_secs(10),
            cancelled: false,
        };
        self.owner.runtime.result(&context, key).map_err(failure)
    }

    // Keep expected bytes private while the separately authorized paid challenge crosses the network.
    fn check(&mut self, id: &[u8; 32], expected: u8) -> Result<(), String> {
        tracing::trace!("Running a separately paid Storage evidence challenge");
        let selection = self
            .cache
            .selection(self.provider_id, "storage-standard", "cr2se.storage", 1)
            .map_err(failure)?;
        let plan = Plan::Check(Box::new(Challenge {
            selection,
            cache: self.cache.clone(),
            challenge: fields([
                ("leaseId", Value::Bytes(id.to_vec())),
                ("offset", Value::Uint64(0)),
            ]),
            spending: peer::requester::Spending {
                maximum: 3,
                payment: Default::default(),
            },
            consent: Arc::new(Ceiling(3)),
        }));
        let pending = self
            .requester
            .request(&self.owner.endpoint, &self.connection, plan)?;
        let Reply::Check(key) = self
            .provider
            .complete(self.requester, &pending)?
            .result
            .map_err(failure)?
        else {
            return Err("Expected a committed Storage check".into());
        };
        let grant = self
            .owner
            .services
            .grant(
                self.requester_id,
                services::authority::Action::Observe,
                16_777_216,
            )
            .map_err(failure)?;
        let context = services::contract::Context {
            version: 1,
            grant: &grant,
            deadline: Instant::now() + Duration::from_secs(10),
            cancelled: false,
        };
        let expected = fields([
            ("leaseId", Value::Bytes(id.to_vec())),
            ("offset", Value::Uint64(0)),
            ("expectedByte", Value::Bytes(vec![expected])),
            ("active", Value::Bool(true)),
        ]);
        let out = self
            .owner
            .runtime
            .check_outcome(&context, key, Some(&expected))
            .map_err(failure)?;
        if out != services::handler::Check::Pass {
            return Err("Storage evidence did not match the private expectation".into());
        }
        Ok(())
    }
}

// Build typed logical objects without JSON numeric coercion.
fn fields<const N: usize>(fields: [(&str, Value); N]) -> Value {
    tracing::trace!("Encoding finite typed Storage exercise fields");
    Value::Object(
        fields
            .into_iter()
            .map(|(name, value)| (name.into(), value))
            .collect(),
    )
}

// Find a typed field in the independently validated operation output.
fn field<'a>(value: &'a Value, name: &str) -> Result<&'a Value, String> {
    tracing::trace!("Inspecting retained Storage exercise result");
    let Value::Object(fields) = value else {
        return Err("Expected Storage object output".into());
    };
    fields
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value)
        .ok_or_else(|| format!("Missing {name}"))
}

// Validate an exact logical integer result without narrowing.
fn number(value: &Value, name: &str) -> Result<u64, String> {
    tracing::trace!("Reading independent Storage exercise integer");
    match field(value, name)? {
        Value::Uint64(n) => Ok(*n),
        _ => Err(format!("Invalid {name}")),
    }
}

// Create a detached upload, reopen and authenticate its same provider, then read, check, renew and remove.
pub(super) fn run() -> Result<(), String> {
    tracing::trace!("Running Storage with independent in-process owners");
    run_with(false)
}

// Drive the same authenticated protocol with either independent owners or a separate provider process.
pub(super) fn run_with(separate: bool) -> Result<(), String> {
    tracing::trace!("Starting the authenticated persistent Storage exercise");
    let root = Directory::new()?;
    let a_identity = root.0.join("a-identity");
    let b_identity = root.0.join("b-identity");
    let (aid, bid) = (node::identity(&a_identity)?, node::identity(&b_identity)?);
    let a_dir = root.0.join("a");
    let b_dir = root.0.join("b");
    let catalog = Arc::new(Catalog(Mutex::new(None)));
    let a_owner = owners::open(&a_dir, aid, bid, true, true, catalog.clone())?;
    let signal = Signal::default();
    let mut a = Node::new(&a_identity, a_owner.endpoint.clone(), signal.clone())?;
    let mut b = Host::start(
        &b_dir,
        &b_identity,
        bid,
        aid,
        true,
        signal.clone(),
        separate,
    )?;
    let (connection, cache) = connect(&mut a, &mut b, &a_owner, bid)?;
    let data = vec![42; 5000];
    let size = 5000;
    let retention = 60;
    let hash = talksphere::modules::cryptography::primitives::hash::sha256(&data, data.len())
        .map_err(failure)?;
    let completed = {
        let mut network = Network {
            requester: &mut a,
            provider: &mut b,
            owner: &a_owner,
            connection,
            cache,
            provider_id: bid,
            requester_id: aid,
        };
        let key = network.invoke(
            fields([
                ("operation", Value::String("store".into())),
                ("sizeBytes", Value::Uint64(size)),
                ("retentionSeconds", Value::Uint64(retention)),
                ("contentHash", Value::Bytes(hash.to_vec())),
                ("data", Value::Bytes(data.clone())),
            ]),
            size * (1 + retention) / 2,
        )?;
        network.result(key)?
    };
    let Value::Bytes(id) = field(&completed.output, "leaseId")? else {
        return Err("Invalid lease ID".into());
    };
    let lease_id = id.as_slice().try_into().map_err(failure)?;
    let mut lease = Lease {
        id: lease_id,
        requester: aid,
        provider: bid,
        size_bytes: size,
        content_hash: hash,
        accepted_at: number(&completed.output, "acceptedAt")?,
        expires_at: number(&completed.output, "expiresAt")?,
        store_price: completed.price,
        state: State::Active,
        accepted_terms: completed.effect,
    };
    *catalog.0.lock().map_err(failure)? = Some(lease.clone());

    b.stop()?;
    b = Host::start(&b_dir, &b_identity, bid, aid, false, signal, separate)?;
    let (connection, cache) = connect(&mut a, &mut b, &a_owner, bid)?;
    let mut network = Network {
        requester: &mut a,
        provider: &mut b,
        owner: &a_owner,
        connection,
        cache,
        provider_id: bid,
        requester_id: aid,
    };
    let id = Value::Bytes(lease_id.to_vec());
    let key = network.invoke(
        fields([
            ("operation", Value::String("retrieve".into())),
            ("leaseId", id.clone()),
            ("offset", Value::Uint64(0)),
            ("length", Value::Uint64(size)),
        ]),
        size / 2,
    )?;
    let retrieved = network.result(key)?;
    if field(&retrieved.output, "data")? != &Value::Bytes(data.clone()) {
        return Err("Retrieved Storage bytes changed".into());
    }
    network.check(&lease_id, data[0])?;
    let key = network.invoke(
        fields([
            ("operation", Value::String("renew".into())),
            ("leaseId", id.clone()),
            ("expectedExpiresAt", Value::Uint64(lease.expires_at)),
            ("additionalSeconds", Value::Uint64(5)),
        ]),
        size * 5 / 2,
    )?;
    let renewed = network.result(key)?;
    lease.expires_at = number(&renewed.output, "expiresAt")?;
    *catalog.0.lock().map_err(failure)? = Some(lease);
    let key = network.invoke(
        fields([
            ("operation", Value::String("remove".into())),
            ("leaseId", id),
        ]),
        2,
    )?;
    if field(&network.result(key)?.output, "removed")? != &Value::Bool(true) {
        return Err("Storage lease removal was not acknowledged".into());
    }
    let expected_balance = 1_000_000 - size * (1 + retention) / 2 - size / 2 - 3 - size * 5 / 2 - 2;
    if network.balances()? != (expected_balance, expected_balance) {
        return Err("Storage accounting differs from the five exact paid operations".into());
    }
    let after_remove = fields([
        ("operation", Value::String("retrieve".into())),
        ("leaseId", Value::Bytes(lease_id.to_vec())),
        ("offset", Value::Uint64(0)),
        ("length", Value::Uint64(size)),
    ]);
    if network.invoke(after_remove, size / 2).is_ok()
        || network.balances()? != (expected_balance, expected_balance)
    {
        return Err("A removed lease accepted new work or charged the original accounts".into());
    }
    b.stop()?;
    Ok(())
}
