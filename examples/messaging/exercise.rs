//! Authenticated paid Messaging across four nodes with independent durable owners: a sender, a recipient and two providers that each
//! hold one replica of the same message. Every node has its own protected identity store, Services, Ledger and Board files; the
//! providers also own placements and journals. Keys never leave Identity: signatures and payload decryption use the same protected
//! identity that authenticates the TCP connection.
use super::{
    failure,
    node::{self, Node, ProtectedIdentity},
    owners::{self, Owners},
};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use talksphere::{
    composition::{
        messaging::{NodeIdentity, SystemClock, SystemRandom},
        server::pump::Signal,
    },
    modules::{
        identity::api::IdentityId,
        ledger,
        messaging::{
            codec::{self, Response},
            encryption,
            ids::{MessageId, RecoveryId, RemovalId},
            metadata::Metadata,
            operation::{
                CheckInput, DiscoverInput, RemoveInput, RenewInput, StatusInput, StoreInput,
            },
            ports::{Clock, Entropy, Signer},
            receipt,
            receive::{Arrival, Inbox, ProviderKey, Recipient},
            signing,
            state::Encoding,
            terms::Terms,
            wire::Remote,
        },
        peer_protocol::engine::endpoint::{
            cache::PeerCache,
            requests::{Plan, Reply},
        },
        services::{self, value::Value},
        transport::handles::ConnectionHandle,
    },
};

type Identity = Arc<
    NodeIdentity<
        talksphere::modules::identity::storage::FileStore,
        talksphere::modules::cryptography::random::SystemEntropy,
    >,
>;

/// Roles by position: two providers, then the sender and the recipient.
const P1: usize = 0;
const P2: usize = 1;
const SENDER: usize = 2;
const RECIPIENT: usize = 3;
const SECRET: &[u8] = b"Hello from the sender, replicated at two providers";
const RETENTION_SECONDS: u64 = 60;

struct Directory(PathBuf);
impl Directory {
    // An atomically allocated owner-only root for this exercise; only this process removes it.
    fn new() -> Result<Self, String> {
        use std::os::unix::fs::DirBuilderExt;
        tracing::trace!("Creating a private Messaging exercise root");
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).map_err(failure)?;
        let suffix = nonce.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let root = std::env::temp_dir().join(format!("talksphere-messaging-{suffix}"));
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
        tracing::trace!("Cleaning up the isolated Messaging exercise");
        if std::fs::remove_dir_all(&self.0).is_err() {
            tracing::warn!("The temporary Messaging directory could not be removed");
        }
    }
}

// One participant: its authenticated transport, owners and identity.
struct Party {
    node: Node,
    owners: Owners,
    identity: Identity,
    id: IdentityId,
}

// A requester's view of one provider: the live connection and, once fetched, the offering's remote.
struct Link {
    connection: ConnectionHandle,
    remote: Option<Remote>,
}

struct Cluster {
    parties: Vec<Party>,
    links: BTreeMap<(usize, usize), Link>,
}

impl Cluster {
    // Run every node until the request's original-stream completion arrives, then return its reply.
    fn complete(
        &mut self,
        pending: &std::sync::mpsc::Receiver<
            talksphere::modules::peer_protocol::engine::endpoint::requests::Completion,
        >,
    ) -> Result<Reply, String> {
        tracing::trace!("Driving all Messaging nodes to one request completion");
        let mut nodes: Vec<&mut Node> = self.parties.iter_mut().map(|p| &mut p.node).collect();
        node::complete(&mut nodes, pending)?.result.map_err(failure)
    }

    // Open one plan from `from` toward `to` and wait for its normally ended exchange.
    fn run_plan(&mut self, from: usize, to: usize, plan: Plan) -> Result<Reply, String> {
        let connection = self
            .links
            .get(&(from, to))
            .ok_or("Missing link")?
            .connection
            .clone();
        let pending = {
            let party = &mut self.parties[from];
            party
                .node
                .request(&party.owners.endpoint, &connection, plan)?
        };
        self.complete(&pending)
    }

    // Run a Messaging plan and read its committed output and price from the requester's own Services records.
    fn work(&mut self, from: usize, to: usize, plan: Plan) -> Result<(Value, u64), String> {
        let Reply::Work(key) = self.run_plan(from, to, plan)? else {
            return Err("Expected an original Messaging work result".into());
        };
        let owners = &self.parties[from].owners;
        let grant = owners
            .services
            .grant(
                self.parties[from].id,
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
        let done = owners.runtime.result(&context, key).map_err(failure)?;
        Ok((done.output, done.price))
    }

    // `holder`'s own balance of credits issued by `issuer`.
    fn owned(&self, holder: usize, issuer: usize) -> Result<u64, String> {
        let owners = &self.parties[holder].owners;
        let grant = owners
            .ledger_authority
            .grant(
                self.parties[holder].id,
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
        Ok(owners
            .account
            .view(&context, self.parties[issuer].id)
            .map_err(failure)?
            .owned)
    }

    // The credits `issuer` has issued to `holder`, as recorded by the issuer's own Ledger.
    fn issued(&self, issuer: usize, holder: usize) -> Result<u64, String> {
        let owners = &self.parties[issuer].owners;
        let grant = owners
            .ledger_authority
            .grant(
                self.parties[issuer].id,
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
        Ok(owners
            .account
            .view(&context, self.parties[holder].id)
            .map_err(failure)?
            .issued)
    }
}

// Build one party: its protected identity file, durable owners and authenticated transport sharing one reactor signal.
fn party(
    root: &std::path::Path,
    name: &str,
    id: IdentityId,
    provider: bool,
    others: &[IdentityId],
    signal: Signal,
) -> Result<Party, String> {
    tracing::trace!("Opening one Messaging exercise party");
    let mut stash: Option<(Owners, ProtectedIdentity)> = None;
    let node = Node::with_identity(
        &root.join(format!("{name}-identity")),
        |identity| {
            let owners = owners::open(&root.join(name), identity.clone(), provider, others)?;
            let endpoint = owners.endpoint.clone();
            stash = Some((owners, identity));
            Ok(Arc::new(endpoint))
        },
        signal,
    )?;
    let (owners, identity) = stash.ok_or("Missing party owners")?;
    Ok(Party {
        node,
        owners,
        identity: Arc::new(NodeIdentity(identity)),
        id,
    })
}

// Connect `from` to `to` over real authenticated TCP and fetch the provider's Board and definition directly from it.
fn link(cluster: &mut Cluster, from: usize, to: usize) -> Result<(), String> {
    tracing::trace!("Connecting a requester to a provider and fetching its terms");
    let (address, provider_id) = (cluster.parties[to].node.address, cluster.parties[to].id);
    let target = cluster.parties[from].node.authenticated + 1;
    let connection = cluster.parties[from].node.connect(address, provider_id)?;
    cluster.links.insert(
        (from, to),
        Link {
            connection,
            remote: None,
        },
    );
    // The connection must be authenticated before any request is opened on it.
    {
        let mut nodes: Vec<&mut Node> = cluster.parties.iter_mut().map(|p| &mut p.node).collect();
        node::until(&mut nodes, |nodes| nodes[from].authenticated >= target)?;
    }
    let cache = Arc::new(
        PeerCache::new(
            cluster.parties[from].owners.registry.clone(),
            Default::default(),
        )
        .map_err(failure)?,
    );
    for plan in [
        Plan::Board(cache.clone()),
        Plan::Definition {
            cache: cache.clone(),
            offering: "messaging-standard".into(),
        },
    ] {
        cluster.run_plan(from, to, plan)?;
    }
    let offering = owners::pinned_offering(provider_id)?;
    let selection = cache
        .selection(
            provider_id,
            &offering.id,
            &offering.service,
            offering.version,
        )
        .map_err(failure)?;
    let terms = Terms::parse(&offering, provider_id).map_err(failure)?;
    cluster.link_mut(from, to)?.remote = Some(Remote::new(terms, selection, cache));
    Ok(())
}

impl Cluster {
    // The mutable link from a requester to a provider.
    fn link_mut(&mut self, from: usize, to: usize) -> Result<&mut Link, String> {
        self.links
            .get_mut(&(from, to))
            .ok_or_else(|| "Missing link".into())
    }
    // The requester's remote for a provider, available after `link`.
    fn remote(&self, from: usize, to: usize) -> Result<&Remote, String> {
        self.links
            .get(&(from, to))
            .and_then(|link| link.remote.as_ref())
            .ok_or_else(|| "Provider terms were not fetched".into())
    }
}

// A failed expectation names the property that did not hold, never a value or key.
fn expect(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(format!("Messaging exercise check failed: {message}"))
    }
}

// Decode a committed Messaging output for the operation that produced it.
fn response(output: &Value) -> Result<Response, String> {
    codec::parse_response(output).map_err(failure)
}

// Store `message` at `provider` for the sender, verifying the exact price and the sender's balance, and remember its size.
fn store_at(
    cluster: &mut Cluster,
    provider: usize,
    message: &talksphere::modules::messaging::metadata::SignedMessage,
) -> Result<talksphere::modules::messaging::operation::StoreOutput, String> {
    tracing::trace!("Storing one message at one provider");
    let before = cluster.owned(SENDER, provider)?;
    let input = StoreInput {
        message: message.clone(),
        retention_seconds: RETENTION_SECONDS,
    };
    let (plan, price) = cluster
        .remote(SENDER, provider)?
        .store(&input)
        .map_err(failure)?;
    let (output, charged) = cluster.work(SENDER, provider, plan)?;
    let Response::Store(stored) = response(&output)? else {
        return Err("Expected a store result".into());
    };
    expect(
        charged == price && price > 0,
        "the store was charged its exact quoted price",
    )?;
    expect(
        cluster.owned(SENDER, provider)? == before - price,
        "the sender's balance fell by the store price",
    )?;
    expect(
        stored.expires_at == stored.accepted_at + RETENTION_SECONDS,
        "the retention is the requested one",
    )?;
    let size = message.payload.len() as u64;
    cluster.parties[SENDER]
        .owners
        .known
        .0
        .lock()
        .map_err(failure)?
        .insert(stored.placement_id, size);
    Ok(stored)
}

/// Run the whole exercise and verify every observable result.
pub(super) fn run() -> Result<(), String> {
    tracing::trace!("Running Messaging with four authenticated nodes");
    let root = Directory::new()?;
    let names = ["p1", "p2", "sender", "recipient"];
    let mut ids = Vec::new();
    for name in names {
        ids.push(node::identity(&root.0.join(format!("{name}-identity")))?);
    }
    let (providers, requesters) = ([ids[P1], ids[P2]], [ids[SENDER], ids[RECIPIENT]]);
    let signal = Signal::default();
    let mut parties = Vec::new();
    for (index, name) in names.iter().enumerate() {
        let provider = index < 2;
        let others: &[IdentityId] = if provider { &requesters } else { &providers };
        parties.push(party(
            &root.0,
            name,
            ids[index],
            provider,
            others,
            signal.clone(),
        )?);
    }
    let mut cluster = Cluster {
        parties,
        links: BTreeMap::new(),
    };
    for from in [SENDER, RECIPIENT] {
        for to in [P1, P2] {
            link(&mut cluster, from, to)?;
        }
    }

    // The sender encrypts to the recipient's published key and signs; the same signed message is stored at both providers.
    let (sender, recipient) = (
        cluster.parties[SENDER].identity.clone(),
        cluster.parties[RECIPIENT].identity.clone(),
    );
    let now = SystemClock.now().map_err(failure)?;
    let mut message_id = MessageId([0; 32]);
    SystemRandom.fill(&mut message_id.0).map_err(failure)?;
    let recipient_key = recipient.0.public().map_err(failure)?.encryption_public_key;
    let metadata = Metadata {
        message_id,
        sender_id: ids[SENDER],
        recipient_id: ids[RECIPIENT],
        sender_created_at: now,
        encoding: Encoding::IdentityEncryptedV1,
        payload_size: 0,
        payload_hash: talksphere::modules::messaging::ids::PayloadHash([0; 32]),
    };
    let payload =
        encryption::encrypt(SECRET, &metadata, &recipient_key, &SystemRandom).map_err(failure)?;
    let size = payload.len() as u64;
    let message = signing::sign_message(
        sender.as_ref(),
        ids[RECIPIENT],
        message_id,
        now,
        Encoding::IdentityEncryptedV1,
        payload,
    )
    .map_err(failure)?;
    let first_store = store_at(&mut cluster, P1, &message)?;
    let second_store = store_at(&mut cluster, P2, &message)?;
    let (first, second) = (first_store.placement_id, second_store.placement_id);
    expect(
        first != second,
        "each provider holds its own placement of the one message",
    )?;
    // The recipient discovers the message at both providers (paying one credit each) and learns its size.
    for provider in [P1, P2] {
        let (plan, _) = cluster
            .remote(RECIPIENT, provider)?
            .discover(DiscoverInput {
                cursor: None,
                limit: 5,
            });
        let (output, _) = cluster.work(RECIPIENT, provider, plan)?;
        let Response::Discover(page) = response(&output)? else {
            return Err("Expected a discovery page".into());
        };
        expect(
            page.messages.len() == 1,
            "the provider lists exactly the stored message",
        )?;
        let entry = &page.messages[0];
        expect(
            entry.message_id == message_id,
            "the listed message is the stored one",
        )?;
        cluster.parties[RECIPIENT]
            .owners
            .known
            .0
            .lock()
            .map_err(failure)?
            .insert(entry.placement_id, entry.payload_size);
    }
    // The recipient recovers from the first provider with a signed request, authenticates, decrypts and deduplicates.
    let inbox = Inbox::new(16);
    let mut accepted = 0;
    for (provider, placement) in [(P1, first), (P2, second)] {
        let mut recovery_id = RecoveryId([0; 32]);
        SystemRandom.fill(&mut recovery_id.0).map_err(failure)?;
        let request = receipt::sign_request(
            recipient.as_ref(),
            ids[provider],
            placement,
            message_id,
            message.metadata.payload_hash,
            recovery_id,
        )
        .map_err(failure)?;
        let before = cluster.owned(RECIPIENT, provider)?;
        let (plan, price) = cluster
            .remote(RECIPIENT, provider)?
            .recover(request, size)
            .map_err(failure)?;
        let (output, charged) = cluster.work(RECIPIENT, provider, plan)?;
        let Response::Recover(recovered) = response(&output)? else {
            return Err("Expected a recovery result".into());
        };
        expect(
            charged == price,
            "the recovery was charged its exact quoted price",
        )?;
        expect(
            cluster.owned(RECIPIENT, provider)? == before - price,
            "the recipient paid for the recovery",
        )?;
        let key = ProviderKey {
            id: ids[provider],
            public_key: cluster.parties[provider].identity.public_key(),
        };
        let own = Recipient {
            id: ids[RECIPIENT],
            encryption_key: recipient.as_ref(),
            encryption_public_key: recipient_key,
        };
        match inbox.accept(&own, &key, &recovered).map_err(failure)? {
            Arrival::New(received) => {
                accepted += 1;
                expect(
                    received.content.as_slice() == SECRET,
                    "the recipient decrypted the exact plaintext",
                )?;
                expect(provider == P1, "the first recovery delivers the message")?;
            }
            Arrival::Duplicate => {
                expect(provider == P2, "only the replica is a duplicate")?;
            }
        }
    }
    expect(
        accepted == 1 && inbox.len() == 1,
        "the replicated message is delivered exactly once",
    )?;
    // The sender sees the recovery receipt at the first provider and extends its retention.
    let (plan, _) = cluster.remote(SENDER, P1)?.status(StatusInput {
        placement_id: first,
    });
    let (output, _) = cluster.work(SENDER, P1, plan)?;
    let Response::Status(status) = response(&output)? else {
        return Err("Expected a status result".into());
    };
    expect(
        status.receipt.is_some(),
        "the sender sees the recovery receipt",
    )?;
    let renew = RenewInput {
        placement_id: first,
        expected_expires_at: first_store.expires_at,
        additional_seconds: 60,
    };
    let (plan, _) = cluster
        .remote(SENDER, P1)?
        .renew(renew, size)
        .map_err(failure)?;
    let (output, _) = cluster.work(SENDER, P1, plan)?;
    let Response::Renew(renewed) = response(&output)? else {
        return Err("Expected a renewal result".into());
    };
    expect(
        renewed.expires_at == first_store.expires_at + 60,
        "the renewal extended the retention",
    )?;
    // A second message that nobody read is removed at the second provider, which releases credits to the sender once.
    let mut second_id = MessageId([0; 32]);
    SystemRandom.fill(&mut second_id.0).map_err(failure)?;
    let retract = signing::sign_message(
        sender.as_ref(),
        ids[RECIPIENT],
        second_id,
        now,
        Encoding::Opaque,
        b"retract me before it is read".to_vec(),
    )
    .map_err(failure)?;
    let stored = store_at(&mut cluster, P2, &retract)?;
    // While unread, the provider still holds the payload byte, so a paid check can be judged with the sender's private expectation.
    let offset = 5;
    let (plan, price) = cluster.remote(SENDER, P2)?.check(&CheckInput {
        placement_id: stored.placement_id,
        offset,
    });
    let before = cluster.owned(SENDER, P2)?;
    let Reply::Check(key) = cluster.run_plan(SENDER, P2, plan)? else {
        return Err("Expected a committed check".into());
    };
    expect(
        cluster.owned(SENDER, P2)? == before - price,
        "the check was charged its exact price",
    )?;
    let expectation = Value::Object(vec![
        (
            "placementId".into(),
            Value::Bytes(stored.placement_id.0.to_vec()),
        ),
        ("offset".into(), Value::Uint64(offset)),
        (
            "expectedByte".into(),
            Value::Bytes(vec![retract.payload[offset as usize]]),
        ),
    ]);
    let grant = cluster.parties[SENDER]
        .owners
        .services
        .grant(
            ids[SENDER],
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
    let outcome = cluster.parties[SENDER]
        .owners
        .runtime
        .check_outcome(&context, key, Some(&expectation))
        .map_err(failure)?;
    expect(
        outcome == services::handler::Check::Pass,
        "the provider returned the byte the sender expected",
    )?;
    let issued = cluster.issued(P2, SENDER)?;
    let mut removal_id = RemovalId([0; 32]);
    SystemRandom.fill(&mut removal_id.0).map_err(failure)?;
    let (plan, _) = cluster.remote(SENDER, P2)?.remove(RemoveInput {
        placement_id: stored.placement_id,
        removal_id,
        expected_expires_at: stored.expires_at,
        minimum_release_credits: 0,
    });
    let (output, _) = cluster.work(SENDER, P2, plan)?;
    let Response::Remove(removed) = response(&output)? else {
        return Err("Expected a removal result".into());
    };
    expect(
        removed.removed && removed.release_credits > 0,
        "the removal released credits",
    )?;
    expect(
        cluster.issued(P2, SENDER)? == issued + removed.release_credits - removed.price,
        "the release credits were issued exactly once, net of the removal price",
    )?;
    Ok(())
}
