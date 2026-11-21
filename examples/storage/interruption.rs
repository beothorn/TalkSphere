//! Real subprocess stream cuts distinguish incomplete upload and unavailable success after commitment.
use super::*;
use crate::process::{Fault, Process};

struct PreparedUpload {
    input: Value,
    price: u64,
}
impl PreparedUpload {
    // Use a detached body large enough that terminating the first body record leaves no complete accepted invocation.
    fn fixture() -> Self {
        let data = vec![81; 5000];
        let hash =
            talksphere::modules::cryptography::primitives::hash::sha256(&data, data.len()).unwrap();
        Self {
            input: fields([
                ("operation", Value::String("store".into())),
                ("sizeBytes", Value::Uint64(5000)),
                ("retentionSeconds", Value::Uint64(60)),
                ("contentHash", Value::Bytes(hash.to_vec())),
                ("data", Value::Bytes(data)),
            ]),
            price: 152_500,
        }
    }
}

// Submit the exact same application object only with fresh explicit local consent.
fn plan(
    cache: Arc<PeerCache>,
    provider: IdentityId,
    upload: &PreparedUpload,
) -> Result<Plan, String> {
    let selection = cache
        .selection(provider, "storage-standard", "cr2se.storage", 1)
        .map_err(failure)?;
    Ok(Plan::Invoke(Box::new(Invocation {
        selection,
        cache,
        input: upload.input.clone(),
        spending: peer::requester::Spending {
            maximum: upload.price,
            payment: Default::default(),
        },
        consent: Arc::new(Ceiling(upload.price)),
    })))
}

// Verify original provider charge after each injected interruption and ensure no replacement lease is made by recovery.
#[test]
fn interrupted_upload_and_lost_result_are_distinct_after_provider_restart() {
    for (fault, charge) in [(Fault::InterruptedUpload, 0), (Fault::LostResult, 152_500)] {
        run(fault, charge).unwrap();
    }
}

// The requesting node sees uncertainty until it can inspect the right original provider after restart.
fn run(fault: Fault, charge: u64) -> Result<(), String> {
    let root = Directory::new()?;
    let ai = root.0.join("ai");
    let bi = root.0.join("bi");
    let b_dir = root.0.join("b");
    let (aid, bid) = (node::identity(&ai)?, node::identity(&bi)?);
    let catalog = Arc::new(Catalog(Mutex::new(None)));
    let a_owner = owners::open(&root.0.join("a"), aid, bid, true, true, catalog)?;
    let signal = Signal::default();
    let mut a = Node::new(&ai, a_owner.endpoint.clone(), signal.clone())?;
    let mut b = Host::Child(Process::start_with_fault(
        &b_dir, &bi, bid, aid, true, fault,
    )?);
    let upload = PreparedUpload::fixture();
    let (connection, cache) = connect(&mut a, &mut b, &a_owner, bid)?;
    let pending = a.request(&a_owner.endpoint, &connection, plan(cache, bid, &upload)?)?;
    let failed = b.complete(&mut a, &pending)?;
    assert!(matches!(failed.result, Err(peer::Error::Unknown)));
    b.stop()?;

    let mut b = Host::Child(Process::start(&b_dir, &bi, bid, aid, false)?);
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
    assert_eq!(network.balances()?, (1_000_000, 1_000_000 - charge));
    let key = network.invoke(upload.input.clone(), upload.price)?;
    let output = network.result(key)?;
    assert_eq!(output.price, upload.price);
    assert_eq!(
        network.balances()?,
        (1_000_000 - upload.price, 1_000_000 - charge - upload.price)
    );
    if fault == Fault::LostResult {
        let Value::Bytes(id) = field(&output.output, "leaseId")? else {
            return Err("Expected active lease identifier".into());
        };
        refuse_foreign(&root.0, &mut b, bid, id.clone(), &output)?;
        assert_eq!(
            network_balances_after_foreign(&mut a, &mut b, &a_owner, aid, bid)?,
            (1_000_000 - upload.price, 1_000_000 - charge - upload.price)
        );
    }
    b.stop()
}

// Charge the provider's completed check once, lose its reply, and reconcile only the requester's local timeout.
#[test]
fn lost_paid_check_reply_preserves_independent_settlement() {
    paid_check_failure(false).unwrap();
}

// A confirmed unavailable authorized byte is an uncharged failure of the check, unlike a lost response.
#[test]
fn confirmed_missing_active_byte_is_uncharged_failure() {
    paid_check_failure(true).unwrap();
}

// Observe the original check after actual Transport failure and compare the two independent books.
fn paid_check_failure(remove_byte: bool) -> Result<(), String> {
    use talksphere::modules::ledger;
    let root = Directory::new()?;
    let ai = root.0.join("ai");
    let bi = root.0.join("bi");
    let b_dir = root.0.join("b");
    let (aid, bid) = (node::identity(&ai)?, node::identity(&bi)?);
    let catalog = Arc::new(Catalog(Mutex::new(None)));
    let a_owner = owners::open(&root.0.join("a"), aid, bid, true, true, catalog.clone())?;
    let mut a = Node::new(&ai, a_owner.endpoint.clone(), Signal::default())?;
    let mut b = Host::Child(Process::start_with_fault(
        &b_dir,
        &bi,
        bid,
        aid,
        true,
        if remove_byte {
            Fault::None
        } else {
            Fault::LostCheckResult
        },
    )?);
    let upload = PreparedUpload::fixture();
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
    let key = network.invoke(upload.input.clone(), upload.price)?;
    let result = network.result(key)?;
    let Value::Bytes(id) = field(&result.output, "leaseId")? else {
        return Err("Missing lease ID".into());
    };
    let Value::Bytes(hash) = field(&result.output, "contentHash")? else {
        return Err("Missing content hash".into());
    };
    let lease = Lease {
        id: id.as_slice().try_into().map_err(failure)?,
        requester: aid,
        provider: bid,
        accepted_terms: result.effect,
        size_bytes: number(&result.output, "sizeBytes")?,
        content_hash: hash.as_slice().try_into().map_err(failure)?,
        accepted_at: number(&result.output, "acceptedAt")?,
        expires_at: number(&result.output, "expiresAt")?,
        store_price: upload.price,
        state: State::Active,
    };
    if remove_byte {
        let name = id
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        std::fs::remove_file(b_dir.join("storage").join(format!("object-{name}")))
            .map_err(failure)?;
    }
    *catalog.0.lock().map_err(failure)? = Some(lease);
    let selection = network
        .cache
        .selection(bid, "storage-standard", "cr2se.storage", 1)
        .map_err(failure)?;
    let plan = Plan::Check(Box::new(Challenge {
        selection,
        cache: network.cache.clone(),
        challenge: fields([
            ("leaseId", Value::Bytes(id.clone())),
            ("offset", Value::Uint64(0)),
        ]),
        spending: peer::requester::Spending {
            maximum: 3,
            payment: Default::default(),
        },
        consent: Arc::new(Ceiling(3)),
    }));
    let pending = network
        .requester
        .request(&a_owner.endpoint, &network.connection, plan)?;
    let reply = network.provider.complete(network.requester, &pending)?;
    assert_eq!(
        reply.result.err(),
        Some(if remove_byte {
            peer::Error::EvidenceUnavailable
        } else {
            peer::Error::Unknown
        })
    );
    let op = reply.scope.association(aid).map_err(failure)?.operation;
    let key = OperationKey {
        requester: op.requester,
        connection: op.connection,
        stream: op.stream,
    };
    let deadline = Instant::now() + Duration::from_secs(10);
    let observe = a_owner
        .services
        .grant(aid, services::authority::Action::Observe, 16_777_216)
        .map_err(failure)?;
    let oc = services::contract::Context {
        version: 1,
        grant: &observe,
        deadline,
        cancelled: false,
    };
    assert_eq!(
        a_owner.runtime.status(&oc, key).map_err(failure)?,
        Some(services::runtime::Phase::Aborted)
    );
    assert_eq!(
        a_owner
            .runtime
            .check_outcome(&oc, key, None)
            .map_err(failure)?,
        services::handler::Check::Inconclusive
    );
    let expected = fields([
        ("leaseId", Value::Bytes(id.clone())),
        ("offset", Value::Uint64(0)),
        ("expectedByte", Value::Bytes(vec![81])),
        ("active", Value::Bool(true)),
    ]);
    if remove_byte {
        assert_eq!(
            a_owner
                .runtime
                .check_outcome(&oc, key, Some(&expected))
                .map_err(failure)?,
            services::handler::Check::Fail
        );
    }
    let grant = a_owner
        .ledger_authority
        .grant(aid, ledger::authority::Action::Read, 16_777_216)
        .map_err(failure)?;
    let available = a_owner
        .account
        .available(
            &ledger::contract::Context {
                version: 1,
                grant: &grant,
                deadline,
                cancelled: false,
            },
            bid,
            true,
        )
        .map_err(failure)?;
    let after_store = 1_000_000 - upload.price;
    assert_eq!(available, after_store);
    assert_eq!(
        network.balances()?,
        (after_store, after_store - if remove_byte { 0 } else { 3 })
    );
    b.stop()
}

// Make an independently authenticated foreign request carrying a known opaque ID. A local expectation cannot establish remote ownership.
fn refuse_foreign(
    root: &std::path::Path,
    host: &mut Host,
    provider: IdentityId,
    id: Vec<u8>,
    output: &services::handler::Completion,
) -> Result<(), String> {
    let identity = root.join("foreign-identity");
    let foreign = node::identity(&identity)?;
    let Value::Bytes(digest) = field(&output.output, "contentHash")? else {
        return Err("Missing fixture hash".into());
    };
    let lease = Lease {
        id: id.as_slice().try_into().map_err(failure)?,
        requester: foreign,
        provider,
        size_bytes: number(&output.output, "sizeBytes")?,
        content_hash: digest.as_slice().try_into().map_err(failure)?,
        accepted_at: number(&output.output, "acceptedAt")?,
        expires_at: number(&output.output, "expiresAt")?,
        accepted_terms: output.effect.clone(),
        store_price: output.price,
        state: State::Active,
    };
    let owner = owners::open(
        &root.join("foreign"),
        foreign,
        provider,
        true,
        true,
        Arc::new(Catalog(Mutex::new(Some(lease)))),
    )?;
    let mut node = Node::new(&identity, owner.endpoint.clone(), Signal::default())?;
    let (connection, cache) = connect(&mut node, host, &owner, provider)?;
    let selection = cache
        .selection(provider, "storage-standard", "cr2se.storage", 1)
        .map_err(failure)?;
    let pending = node.request(
        &owner.endpoint,
        &connection,
        Plan::Invoke(Box::new(Invocation {
            selection,
            cache,
            input: fields([
                ("operation", Value::String("retrieve".into())),
                ("leaseId", Value::Bytes(id)),
                ("offset", Value::Uint64(0)),
                ("length", Value::Uint64(1)),
            ]),
            spending: peer::requester::Spending {
                maximum: 1,
                payment: Default::default(),
            },
            consent: Arc::new(Ceiling(1)),
        })),
    )?;
    let reply = host.complete(&mut node, &pending)?;
    assert!(matches!(reply.result, Err(peer::Error::Denied)));
    Ok(())
}

// Demonstrate the original authenticated owner remains usable after a separate peer's refused request.
fn network_balances_after_foreign(
    a: &mut Node,
    b: &mut Host,
    owner: &Owners,
    requester: IdentityId,
    provider: IdentityId,
) -> Result<(u64, u64), String> {
    let (connection, cache) = connect(a, b, owner, provider)?;
    Network {
        requester: a,
        provider: b,
        owner,
        connection,
        cache,
        provider_id: provider,
        requester_id: requester,
    }
    .balances()
}

// A delayed detached result and a small sibling request share one actual endpoint without blocking each other.
#[test]
fn slow_storage_result_does_not_block_sibling_stream() {
    slow_result().unwrap();
}

// Gate the original response at a deterministic provider boundary while verifying an independent balance response.
fn slow_result() -> Result<(), String> {
    let root = Directory::new()?;
    let ai = root.0.join("ai");
    let bi = root.0.join("bi");
    let b_dir = root.0.join("b");
    let (aid, bid) = (node::identity(&ai)?, node::identity(&bi)?);
    let catalog = Arc::new(Catalog(Mutex::new(None)));
    let a_owner = owners::open(&root.0.join("a"), aid, bid, true, true, catalog.clone())?;
    let mut a = Node::new(&ai, a_owner.endpoint.clone(), Signal::default())?;
    let mut b = Host::Child(Process::start_with_fault(
        &b_dir,
        &bi,
        bid,
        aid,
        true,
        Fault::SlowRetrieve,
    )?);
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
    let upload = PreparedUpload::fixture();
    let key = network.invoke(upload.input, upload.price)?;
    let result = network.result(key)?;
    let Value::Bytes(id) = field(&result.output, "leaseId")? else {
        return Err("Missing lease ID".into());
    };
    let Value::Bytes(hash) = field(&result.output, "contentHash")? else {
        return Err("Missing hash".into());
    };
    *catalog.0.lock().map_err(failure)? = Some(Lease {
        id: id.as_slice().try_into().map_err(failure)?,
        requester: aid,
        provider: bid,
        accepted_terms: result.effect,
        size_bytes: 5000,
        content_hash: hash.as_slice().try_into().map_err(failure)?,
        accepted_at: number(&result.output, "acceptedAt")?,
        expires_at: number(&result.output, "expiresAt")?,
        store_price: upload.price,
        state: State::Active,
    });
    let selection = network
        .cache
        .selection(bid, "storage-standard", "cr2se.storage", 1)
        .map_err(failure)?;
    let pending = network.requester.request(
        &a_owner.endpoint,
        &network.connection,
        Plan::Invoke(Box::new(Invocation {
            selection,
            cache: network.cache.clone(),
            input: fields([
                ("operation", Value::String("retrieve".into())),
                ("leaseId", Value::Bytes(id.clone())),
                ("offset", Value::Uint64(0)),
                ("length", Value::Uint64(5000)),
            ]),
            spending: peer::requester::Spending {
                maximum: 2500,
                payment: Default::default(),
            },
            consent: Arc::new(Ceiling(2500)),
        })),
    )?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while !b_dir.with_extension("result-ready").exists() {
        if Instant::now() >= deadline {
            return Err("Provider did not reach the original result barrier".into());
        }
        network.requester.poll()?;
        std::thread::sleep(Duration::from_millis(5));
    }
    let after_store = 1_000_000 - upload.price;
    assert_eq!(network.balances()?, (after_store, after_store - 2500));
    assert!(matches!(
        pending.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    ));
    std::fs::write(b_dir.with_extension("allow-result"), b"release").map_err(failure)?;
    let Reply::Work(key) = network
        .provider
        .complete(network.requester, &pending)?
        .result
        .map_err(failure)?
    else {
        return Err("Expected original retrieval after releasing the delivery barrier".into());
    };
    assert_eq!(network.result(key)?.price, 2500);
    assert_eq!(
        network.balances()?,
        (after_store - 2500, after_store - 2500)
    );
    b.stop()
}
