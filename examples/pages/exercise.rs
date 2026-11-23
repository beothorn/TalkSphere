//! Authenticated paid hosted pages across four nodes with independent durable owners: a publisher that never accepts a connection (like a phone behind NAT),
//! two hosts that each store the same signed page, and a reader that fetches and verifies it from any host. Every node has its own protected identity store,
//! Services, Ledger and Board files; the hosts also own placements and journals. Keys never leave Identity: signatures and the decryption of a restricted
//! page use the same protected identity that authenticates the TCP connection.
//!
//! Process model (deliberate): all four nodes run in ONE OS process, each driven by its own reactor over real loopback TCP, with separate identity files,
//! stores and listeners. Nothing is shared between nodes except the process address space and the readiness signal, and every byte between them crosses an
//! authenticated socket. The publisher only dials out: it opens connections to the hosts and no one connects to it. The limit is that this exercise cannot show
//! a host process crashing or restarting; that is covered by the `hosted_publications::service::crash_tests` and, for Storage, by the Storage example's
//! child-process provider. A host "going away" is simulated by the reader not asking it.
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
        hosted_publications::{
            codec::{self, Response},
            ids::{PlacementId, PublicationId, RemovalId},
            latest::select_latest,
            metadata::{Kind, Publication},
            operation::{
                CheckRequest, GetRequest, ListEntry, ListRequest, RemoveRequest, RenewRequest,
                StoreRequest,
            },
            terms::Terms,
            verify::Bounds,
            wire::Remote,
        },
        identity::api::IdentityId,
        ledger,
        messaging::ports::{Clock, Entropy},
        pages::{
            hosted::{self, Audience, Built, Fetch, PageDraft, Reader, Resource},
            media::MediaType,
            path::CanonicalPath,
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

/// Roles by position: two hosts, then the publisher and the reader.
const H1: usize = 0;
const H2: usize = 1;
const PUBLISHER: usize = 2;
const READER: usize = 3;
const RETENTION_SECONDS: u64 = 60;
/// The most resources a manifest may name in this demonstration.
const MAXIMUM_RESOURCES: usize = 16;

struct Directory(PathBuf);
impl Directory {
    // An atomically allocated owner-only root for this exercise; only this process removes it.
    fn new() -> Result<Self, String> {
        use std::os::unix::fs::DirBuilderExt;
        tracing::trace!("Creating a private hosted pages exercise root");
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).map_err(failure)?;
        let suffix = nonce.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let root = std::env::temp_dir().join(format!("talksphere-pages-{suffix}"));
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
        tracing::trace!("Cleaning up the isolated hosted pages exercise");
        if std::fs::remove_dir_all(&self.0).is_err() {
            tracing::warn!("The temporary hosted pages directory could not be removed");
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
        tracing::trace!("Driving all hosted pages nodes to one request completion");
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

    // Run a hosted pages plan and read its committed output and price from the requester's own Services records.
    fn work(&mut self, from: usize, to: usize, plan: Plan) -> Result<(Value, u64), String> {
        let Reply::Work(key) = self.run_plan(from, to, plan)? else {
            return Err("Expected an original hosted pages work result".into());
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
    tracing::trace!("Opening one hosted pages exercise party");
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
            offering: "hosted-publication-standard".into(),
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
        Err(format!("Hosted pages exercise check failed: {message}"))
    }
}

// Decode a committed hosted publication output for the operation that produced it.
fn response(output: &Value) -> Result<Response, String> {
    codec::parse_response(output).map_err(failure)
}

/// Where one publication is held at one host.
#[derive(Clone, Copy)]
struct Held {
    placement: PlacementId,
    expires_at: u64,
    size: u64,
}

// Store every publication of a page at `host` for the publisher at the exact quoted price, checking the charge and the balance, and remember each placement.
fn store_page(
    cluster: &mut Cluster,
    host: usize,
    built: &Built,
    held: &mut BTreeMap<(usize, PublicationId), Held>,
) -> Result<u64, String> {
    tracing::trace!("Storing one page snapshot at one host");
    let mut spent = 0;
    let before = cluster.owned(PUBLISHER, host)?;
    for publication in built.resources.iter().chain(std::iter::once(&built.page)) {
        let request = StoreRequest {
            publication: publication.clone(),
            retention_seconds: RETENTION_SECONDS,
        };
        let (plan, price) = cluster
            .remote(PUBLISHER, host)?
            .store(&request)
            .map_err(failure)?;
        let (output, charged) = cluster.work(PUBLISHER, host, plan)?;
        let Response::Store(stored, _) = response(&output)? else {
            return Err("Expected a store result".into());
        };
        expect(
            charged == price && price > 0,
            "the store was charged its exact quoted price",
        )?;
        expect(
            stored.expires_at == stored.accepted_at + RETENTION_SECONDS,
            "the retention is the requested one",
        )?;
        let size = publication.metadata.content_size;
        cluster.parties[PUBLISHER]
            .owners
            .known
            .0
            .lock()
            .map_err(failure)?
            .insert(stored.placement_id, size);
        held.insert(
            (host, publication.metadata.publication_id),
            Held {
                placement: stored.placement_id,
                expires_at: stored.expires_at,
                size,
            },
        );
        spent += price;
    }
    expect(
        cluster.owned(PUBLISHER, host)? == before - spent,
        "the publisher's balance fell by exactly the store prices",
    )?;
    Ok(spent)
}

// The reader lists a host's pages, newest first, and pays the fixed list price.
fn list_pages(cluster: &mut Cluster, host: usize) -> Result<Vec<ListEntry>, String> {
    let publisher = cluster.parties[PUBLISHER].id;
    let (plan, price) = cluster.remote(READER, host)?.list(ListRequest {
        publisher_id: publisher,
        kind: Kind::Page,
        before: None,
        limit: 10,
    });
    let before = cluster.owned(READER, host)?;
    let (output, charged) = cluster.work(READER, host, plan)?;
    let Response::List(page, _) = response(&output)? else {
        return Err("Expected a list result".into());
    };
    expect(charged == price, "the list was charged its fixed price")?;
    expect(
        cluster.owned(READER, host)? == before - price,
        "the reader paid for the list",
    )?;
    Ok(page.entries)
}

// The reader gets one publication from one host for the exact price of its known size.
fn get_publication(
    cluster: &mut Cluster,
    host: usize,
    id: PublicationId,
    size: u64,
) -> Result<Publication, String> {
    let publisher = cluster.parties[PUBLISHER].id;
    let request = GetRequest {
        publisher_id: publisher,
        publication_id: id,
        maximum_bytes: 4096,
    };
    let (plan, price) = cluster
        .remote(READER, host)?
        .get(request, size)
        .map_err(failure)?;
    let before = cluster.owned(READER, host)?;
    let (output, charged) = cluster.work(READER, host, plan)?;
    let Response::Get(got, _) = response(&output)? else {
        return Err("Expected a get result".into());
    };
    expect(
        charged == price,
        "the get was charged its exact quoted price",
    )?;
    expect(
        cluster.owned(READER, host)? == before - price,
        "the reader paid for the get",
    )?;
    Ok(got.publication)
}

// Publications the reader fetched, served to the assembly without any further paid request.
struct Fetched(BTreeMap<PublicationId, Publication>);
impl Fetch for Fetched {
    fn publication(
        &self,
        _: IdentityId,
        id: PublicationId,
    ) -> Result<Option<Publication>, talksphere::modules::pages::error::Error> {
        Ok(self.0.get(&id).cloned())
    }
}

// Read the newest page `hosts` offer: list at the first host that answers, fetch the manifest, then fetch each named resource from the hosts in order
// (a host that fails is skipped), and assemble the page, verifying every signature, size and hash itself.
fn read_page(
    cluster: &mut Cluster,
    hosts: &[usize],
    reader: &Reader<'_>,
) -> Result<hosted::Page, String> {
    tracing::trace!("Reading the newest page from the hosts that answer");
    let publisher = cluster.parties[PUBLISHER].id;
    let mut newest = None;
    for &host in hosts {
        let entries = list_pages(cluster, host)?;
        if let Some(entry) = entries.first() {
            newest = Some((host, *entry));
            break;
        }
    }
    let (host, entry) = newest.ok_or("No host listed a page")?;
    let manifest_publication =
        get_publication(cluster, host, entry.publication_id, entry.content_size)?;
    let manifest =
        hosted::manifest_of(&manifest_publication, publisher, reader).map_err(failure)?;
    let mut fetched = BTreeMap::new();
    for item in manifest.entries() {
        for &source in hosts {
            if let Ok(publication) =
                get_publication(cluster, source, item.publication_id, item.size)
            {
                fetched.insert(item.publication_id, publication);
                break;
            }
        }
    }
    hosted::assemble(&manifest_publication, publisher, reader, &Fetched(fetched)).map_err(failure)
}

// A demonstration resource.
fn resource(path: &str, media: &str, content: &str) -> Result<Resource, String> {
    Ok(Resource {
        path: CanonicalPath::parse(path.as_bytes()).map_err(failure)?,
        media_type: MediaType::parse(media).map_err(failure)?,
        content: content.as_bytes().to_vec(),
    })
}

/// Run the whole exercise and verify every observable result.
pub(super) fn run() -> Result<(), String> {
    tracing::trace!("Running hosted pages with four authenticated nodes");
    let root = Directory::new()?;
    let names = ["h1", "h2", "publisher", "reader"];
    let mut ids = Vec::new();
    for name in names {
        ids.push(node::identity(&root.0.join(format!("{name}-identity")))?);
    }
    let (hosts, requesters) = ([ids[H1], ids[H2]], [ids[PUBLISHER], ids[READER]]);
    let signal = Signal::default();
    let mut parties = Vec::new();
    for (index, name) in names.iter().enumerate() {
        let host = index < 2;
        let others: &[IdentityId] = if host { &requesters } else { &hosts };
        parties.push(party(
            &root.0,
            name,
            ids[index],
            host,
            others,
            signal.clone(),
        )?);
    }
    let mut cluster = Cluster {
        parties,
        links: BTreeMap::new(),
    };
    // Only the requesters dial: the hosts accept connections and the publisher never does.
    for from in [PUBLISHER, READER] {
        for to in [H1, H2] {
            link(&mut cluster, from, to)?;
        }
    }
    let publisher = cluster.parties[PUBLISHER].identity.clone();
    let reader_identity = cluster.parties[READER].identity.clone();
    let reader_key = reader_identity
        .0
        .public()
        .map_err(failure)?
        .encryption_public_key;
    let bounds = Bounds {
        maximum_audience: 4,
        maximum_content_bytes: 4096,
    };
    let mut held: BTreeMap<(usize, PublicationId), Held> = BTreeMap::new();
    let now = SystemClock.now().map_err(failure)?;

    // The publisher signs one public page snapshot and stores the same signed publications at both hosts.
    let public_page = hosted::build(
        publisher.as_ref(),
        PageDraft {
            sequence: 1,
            published_at: now,
            audience: Audience::Public,
            resources: vec![
                resource("/", "text/html", "<h1>My page</h1><link href=/style.css>")?,
                resource("/style.css", "text/css", "h1{color:teal}")?,
                resource("/img/note.txt", "text/plain", "hosted, not served by me")?,
            ],
        },
        MAXIMUM_RESOURCES,
        &SystemRandom,
    )
    .map_err(failure)?;
    store_page(&mut cluster, H1, &public_page, &mut held)?;
    store_page(&mut cluster, H2, &public_page, &mut held)?;

    // The publisher is now offline for the rest of the exercise except for the explicit later steps; the reader needs only the hosts.
    let public_reader = Reader {
        bounds,
        maximum_resources: MAXIMUM_RESOURCES,
        key: None,
    };
    let page = read_page(&mut cluster, &[H1, H2], &public_reader)?;
    expect(
        page.is_complete() && page.resources.len() == 3,
        "the reader assembled the complete page from the hosts",
    )?;
    let root_resource = page
        .resource(&CanonicalPath::root())
        .ok_or("Missing root resource")?;
    expect(
        root_resource.media_type.is_html()
            && root_resource.content.starts_with(b"<h1>My page</h1>"),
        "the verified root is the publisher's HTML",
    )?;
    // One host goes away: the same page is still complete from the other host alone.
    let again = read_page(&mut cluster, &[H2], &public_reader)?;
    expect(
        again.is_complete() && again.resources == page.resources,
        "the page is unchanged and complete from one host alone",
    )?;

    // The publisher then stores a newer page for one reader only, encrypted to the reader's published key, at one host.
    let private_page = hosted::build(
        publisher.as_ref(),
        PageDraft {
            sequence: 2,
            published_at: now + 1,
            audience: Audience::Restricted(vec![(ids[READER], reader_key)]),
            resources: vec![
                resource("/", "text/html", "<h1>For you only</h1>")?,
                resource("/secret.txt", "text/plain", "only the reader can open this")?,
            ],
        },
        MAXIMUM_RESOURCES,
        &SystemRandom,
    )
    .map_err(failure)?;
    store_page(&mut cluster, H2, &private_page, &mut held)?;
    let private_reader = Reader {
        bounds,
        maximum_resources: MAXIMUM_RESOURCES,
        key: Some((reader_identity.as_ref(), reader_key)),
    };
    let newest = read_page(&mut cluster, &[H2], &private_reader)?;
    expect(
        newest.sequence == 2 && newest.is_complete(),
        "the reader opened the newer restricted page",
    )?;
    expect(
        newest
            .resource(&CanonicalPath::parse(b"/secret.txt").map_err(failure)?)
            .is_some_and(|r| r.content == b"only the reader can open this"),
        "the decrypted resource is the publisher's plaintext",
    )?;
    expect(
        hosted::manifest_of(&private_page.page, ids[PUBLISHER], &public_reader).is_err(),
        "a reader without the key learns nothing from the restricted page",
    )?;
    // Across both hosts the reader takes the highest verified sequence, as the best known and never as proven latest.
    let mut candidates = Vec::new();
    for host in [H1, H2] {
        for entry in list_pages(&mut cluster, host)? {
            candidates.push(get_publication(
                &mut cluster,
                host,
                entry.publication_id,
                entry.content_size,
            )?);
        }
    }
    let selection =
        select_latest(ids[PUBLISHER], Kind::Page, &candidates, bounds).map_err(failure)?;
    expect(
        selection
            .best
            .as_ref()
            .is_some_and(|p| p.metadata.sequence == 2)
            && !selection.is_equivocating()
            && !selection.is_proven_latest(),
        "the best-known page across both hosts is the newer one",
    )?;
    let only_first = list_pages(&mut cluster, H1)?;
    expect(
        only_first.len() == 1 && only_first[0].sequence == 1,
        "the host that never stored the newer page still shows the older one",
    )?;

    // The publisher extends the first page's retention at one host, challenges one byte of a resource at the other, and removes the old page there.
    let manifest = (H1, public_page.page.metadata.publication_id);
    let slot = *held.get(&manifest).ok_or("Missing manifest placement")?;
    let renew = RenewRequest {
        placement_id: slot.placement,
        expected_expires_at: slot.expires_at,
        additional_seconds: 60,
    };
    let (plan, price) = cluster
        .remote(PUBLISHER, H1)?
        .renew(renew, slot.size)
        .map_err(failure)?;
    let before = cluster.owned(PUBLISHER, H1)?;
    let (output, charged) = cluster.work(PUBLISHER, H1, plan)?;
    let Response::Renew(renewed, _) = response(&output)? else {
        return Err("Expected a renewal result".into());
    };
    expect(
        charged == price && renewed.expires_at == slot.expires_at + 60,
        "the renewal extended the retention at its exact price",
    )?;
    expect(
        cluster.owned(PUBLISHER, H1)? == before - price,
        "the publisher paid for the renewal",
    )?;
    let resource_publication = &public_page.resources[0];
    let probe = *held
        .get(&(H2, resource_publication.metadata.publication_id))
        .ok_or("Missing resource placement")?;
    let offset = 3;
    let (plan, price) = cluster.remote(PUBLISHER, H2)?.check(&CheckRequest {
        placement_id: probe.placement,
        offset,
    });
    let before = cluster.owned(PUBLISHER, H2)?;
    let Reply::Check(key) = cluster.run_plan(PUBLISHER, H2, plan)? else {
        return Err("Expected a committed check".into());
    };
    expect(
        cluster.owned(PUBLISHER, H2)? == before - price,
        "the check was charged its exact price",
    )?;
    let expectation = Value::Object(vec![
        (
            "placementId".into(),
            Value::Bytes(probe.placement.0.to_vec()),
        ),
        ("offset".into(), Value::Uint64(offset)),
        (
            "expectedByte".into(),
            Value::Bytes(vec![resource_publication.content[offset as usize]]),
        ),
    ]);
    let grant = cluster.parties[PUBLISHER]
        .owners
        .services
        .grant(
            ids[PUBLISHER],
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
    let outcome = cluster.parties[PUBLISHER]
        .owners
        .runtime
        .check_outcome(&context, key, Some(&expectation))
        .map_err(failure)?;
    expect(
        outcome == services::handler::Check::Pass,
        "the host returned the byte the publisher expected",
    )?;
    let old = *held
        .get(&(H2, public_page.page.metadata.publication_id))
        .ok_or("Missing old page placement")?;
    let issued = cluster.issued(H2, PUBLISHER)?;
    let mut removal_id = RemovalId([0; 32]);
    SystemRandom.fill(&mut removal_id.0).map_err(failure)?;
    let (plan, _) = cluster.remote(PUBLISHER, H2)?.remove(RemoveRequest {
        placement_id: old.placement,
        removal_id,
        expected_expires_at: old.expires_at,
        minimum_release_credits: 0,
    });
    let (output, _) = cluster.work(PUBLISHER, H2, plan)?;
    let Response::Remove(removed) = response(&output)? else {
        return Err("Expected a removal result".into());
    };
    expect(
        removed.removed && removed.release_credits > 0,
        "the removal released credits",
    )?;
    expect(
        cluster.issued(H2, PUBLISHER)? == issued + removed.release_credits - removed.price,
        "the release credits were issued exactly once, net of the removal price",
    )?;
    let remaining = list_pages(&mut cluster, H2)?;
    expect(
        remaining.len() == 1 && remaining[0].sequence == 2,
        "after the removal the host lists only the newer page",
    )?;
    Ok(())
}
