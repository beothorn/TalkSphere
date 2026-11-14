//! Three real node processes exercise metadata, work/checks and independent issuer controls; issuer restart retains actual durable replay.
use super::*;
use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Child, Command, Stdio},
    sync::mpsc,
};
const CHILD: &str = "TALKSPHERE_PEER_NODE_FIXTURE";
const HELPER: &str =
    "modules::peer_protocol::engine::endpoint::tests::network::processes::node_process";
struct Process {
    child: Child,
    stop: PathBuf,
    address: std::net::SocketAddr,
}
impl Process {
    // Spawn an independently executing test-owned node and await bounded readiness on stdout, not a startup delay.
    fn start(root: &Path, name: &str, local: IdentityId, other: IdentityId, issuer: bool) -> Self {
        let config = root.join(format!("{name}-config.json"));
        let stop = root.join(format!("{name}-stop"));
        let _ = std::fs::remove_file(&stop);
        std::fs::write(&config, serde_json::json!({"identity":root.join(name),"local":identity::text::format(local),"other":identity::text::format(other),"issuer":issuer,"ledger":root.join(format!("{name}-ledger")),"stop":stop}).to_string()).unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", HELPER, "--nocapture", "--test-threads=1"])
            .env(CHILD, &config)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (ready, receiver) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut announced = false;
            for _ in 0..16 {
                let mut line = String::new();
                let count = reader.by_ref().take(1024).read_line(&mut line).unwrap();
                if count == 0 {
                    break;
                }
                if let Some((_, address)) = line.split_once("PEER_READY ")
                    && !announced
                {
                    let _ = ready.send(address.trim().parse().unwrap());
                    announced = true;
                }
            }
        });
        let address = match receiver.recv_timeout(Duration::from_secs(15)) {
            Ok(address) => address,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("node readiness: {error}")
            }
        };
        assert_ne!(child.id(), std::process::id());
        Self {
            child,
            stop,
            address,
        }
    }
    // Graceful process exit closes protected store locks before a fresh node opens the same credential and Ledger.
    fn stop(&mut self) {
        std::fs::write(&self.stop, b"stop").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success());
                return;
            }
            assert!(Instant::now() < deadline, "child failed to stop");
            std::thread::yield_now();
        }
    }
}
impl Drop for Process {
    // Reap a failed test's child before its owned fixture directories are removed.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
// This helper is inert in ordinary runs and starts only from this fixture's explicit bounded local configuration.
#[test]
fn node_process() {
    let Some(config) = std::env::var_os(CHILD) else {
        return;
    };
    let bytes = std::fs::read(config).unwrap();
    assert!(bytes.len() < 4096);
    let config: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let local = identity::text::parse(config["local"].as_str().unwrap()).unwrap();
    let other = identity::text::parse(config["other"].as_str().unwrap()).unwrap();
    let world = World::with_identities(local, other, false, Arc::new(Echo::with_checks(false)));
    let factory: Arc<dyn Factory> = if config["issuer"] == true {
        issuer_endpoint(&world, Path::new(config["ledger"].as_str().unwrap()))
    } else {
        Arc::new(endpoint_world(&world))
    };
    let mut node = Node::new(
        Path::new(config["identity"].as_str().unwrap()),
        factory,
        Signal::default(),
    );
    println!("PEER_READY {}", node.address);
    std::io::stdout().flush().unwrap();
    let deadline = Instant::now() + Duration::from_secs(90);
    let stop = Path::new(config["stop"].as_str().unwrap());
    while !stop.exists() {
        assert!(Instant::now() < deadline, "parent did not stop node");
        node.poll();
        node.signal.wait(Duration::from_millis(20)).unwrap();
    }
    node.bridge.stop();
}
// The third issuer publishes no service capability and uses a real protected permanent transfer/claim book.
pub(super) fn issuer_endpoint(world: &World, path: &Path) -> Arc<dyn Factory> {
    let authority = Arc::new(ledger::authority::Authority::new());
    let admin = authority
        .grant(
            world.local,
            ledger::authority::Action::Administer,
            16_777_216,
        )
        .unwrap();
    let context = peer::test_support::world::ledger_context(&admin);
    let mut store = ledger::linux::FileStore::open(path, Default::default(), true).unwrap();
    use ledger::persistence::Persistence;
    let owner = if store.load().unwrap().is_some() {
        let read = authority
            .grant(world.local, ledger::authority::Action::Read, 16_777_216)
            .unwrap();
        ledger::state::Ledger::open(
            world.local,
            authority.clone(),
            store,
            Default::default(),
            &peer::test_support::world::ledger_context(&read),
        )
        .unwrap()
    } else {
        let ledger = ledger::state::Ledger::initialize(
            world.local,
            authority.clone(),
            store,
            Default::default(),
            &context,
            1000,
            100,
        )
        .unwrap();
        ledger.grant(&context, world.requester, 100).unwrap();
        ledger
    };
    let mut config = endpoint_config(world);
    // Keep the independent registry/Services owners for required contracts; no service is published by this issuer profile.
    let publish = world
        .board_authority
        .grant(world.local, board::authority::Action::Publish, 16_777_216)
        .unwrap();
    let prepared = board::prepared::Prepared::new(
        world.local,
        br#"{"version":1,"providedServices":[],"wantedServices":[]}"#,
        &std::collections::BTreeMap::new(),
        Default::default(),
        world.registry.as_ref(),
    )
    .unwrap();
    world
        .board
        .publish(
            &board_context(&publish, Instant::now() + Duration::from_secs(30)),
            prepared,
        )
        .unwrap();
    config.transfer_policy = Arc::new(peer::ledger_control::transfer::Fixed(2));
    Arc::new(
        Endpoint::new(Config {
            local: config.local,
            authority: config.authority,
            services: config.services,
            board: config.board,
            board_authority: config.board_authority,
            registry: config.registry,
            runtime: config.runtime,
            ledger: Arc::new(owner),
            ledger_authority: authority,
            gate: config.gate,
            transfer_policy: config.transfer_policy,
            claims: None,
            wanted: None,
            resources: config.resources,
            limits: config.limits,
            service_limits: config.service_limits,
        })
        .unwrap(),
    )
}
// Signing is done through a real protected Identity capability before Transport acquires that store's exclusive lock.
pub(super) fn signed(
    path: &Path,
    order: ledger::transfer::Order,
) -> peer::ledger_control::transfer_client::Signed {
    let limits = identity::config::Limits::new(256, 8, 4096, 8192, 4096).unwrap();
    let (store, admin) = identity::storage::FileStore::open(path, limits).unwrap();
    let mut provider = identity::provider::Identity::new(store, SystemEntropy, limits, 1).unwrap();
    let (handle, public) = provider.load(&admin).unwrap();
    let grant = provider
        .grant::<identity::authority::Sign>(&admin, &handle, 121)
        .unwrap();
    let signature = provider
        .sign(&handle, &grant, &order.encode().unwrap())
        .unwrap();
    peer::ledger_control::transfer_client::Signed {
        order,
        key: public.identity.signing_public_key(),
        signature,
        maximum_fee: 2,
    }
}

// Three independent processes authenticate actual credentials, commit detached work/checks, and retain issuer transfers after restart.
#[test]
fn three_node_processes_complete_work_and_durable_issuer_replay() {
    let root = Directory::new();
    let a_path = root.0.join("a");
    let a_id = identity(&a_path);
    let b_id = identity(&root.0.join("b"));
    let c_id = identity(&root.0.join("c"));
    let order = ledger::transfer::Order {
        issuer: c_id,
        owner: a_id,
        new_owner: b_id,
        transfer_id: [71; 16],
        amount: 9,
    };
    let signed = signed(&a_path, order);
    let replay = peer::ledger_control::transfer_client::Signed {
        order: signed.order.clone(),
        key: signed.key,
        signature: signed.signature,
        maximum_fee: 0,
    };
    let mut b = Process::start(&root.0, "b", b_id, a_id, false);
    let mut c = Process::start(&root.0, "c", c_id, a_id, true);
    let world = World::with_identities(a_id, b_id, true, Arc::new(Echo::with_checks(false)));
    let endpoint = endpoint_world(&world);
    let mut a = Node::new(&a_path, Arc::new(endpoint.clone()), Signal::default());
    let b_connection = a.connect(b.address, b_id);
    let c_connection = a.connect(c.address, c_id);
    until(&mut [&mut a], |nodes| nodes[0].authenticated == 2);
    let cache =
        Arc::new(cache::PeerCache::new(world.registry.clone(), Default::default()).unwrap());
    for plan in [
        requests::Plan::Board(cache.clone()),
        requests::Plan::Definition {
            cache: cache.clone(),
            offering: "fixture".into(),
        },
    ] {
        let result = a.request(&endpoint, &b_connection, plan);
        assert!(matches!(
            complete(&mut [&mut a], &result).result,
            Ok(requests::Reply::Metadata(_))
        ));
    }
    let offering = world.family.offering(b_id);
    let selected = cache
        .selection(b_id, &offering.id, &offering.service, offering.version)
        .unwrap();
    let result = a.request(
        &endpoint,
        &b_connection,
        requests::Plan::Invoke(Box::new(requests::Invocation {
            selection: selected.clone(),
            cache: cache.clone(),
            input: Value::String("detached 🦀".repeat(1500)),
            spending: peer::requester::Spending {
                maximum: 7,
                payment: Default::default(),
            },
            consent: Arc::new(requests::Ceiling(7)),
        })),
    );
    let reply = complete(&mut [&mut a], &result);
    let Ok(requests::Reply::Work(key)) = reply.result else {
        panic!("real process work: {:?}", reply.cause)
    };
    let observe = world
        .services
        .grant(a_id, services::authority::Action::Observe, 16_777_216)
        .unwrap();
    assert_eq!(
        world
            .runtime
            .completed(&service_context(&observe), key)
            .unwrap()
            .1
            .output,
        Value::String("detached 🦀".repeat(1500))
    );
    let result = a.request(
        &endpoint,
        &b_connection,
        requests::Plan::Check(Box::new(requests::Challenge {
            selection: selected,
            cache,
            challenge: Value::Uint64(1),
            spending: peer::requester::Spending {
                maximum: 3,
                payment: Default::default(),
            },
            consent: Arc::new(requests::Ceiling(3)),
        })),
    );
    assert!(matches!(
        complete(&mut [&mut a], &result).result,
        Ok(requests::Reply::Check(_))
    ));
    let result = a.request(
        &endpoint,
        &c_connection,
        requests::Plan::Transfer {
            signed: Box::new(signed),
            consent: Arc::new(requests::ExactFee(2)),
        },
    );
    let reply = complete(&mut [&mut a], &result);
    let Ok(requests::Reply::Transfer(transfer)) = reply.result else {
        panic!("issuer transfer")
    };
    assert_eq!(transfer.proof().result().received(), Ok(7));
    // A fresh process opens the original credential and permanent issuer book, not a copied heap or reconstructed balance.
    c.stop();
    drop(c);
    let mut c = Process::start(&root.0, "c", c_id, a_id, true);
    let connection = a.connect(c.address, c_id);
    until(&mut [&mut a], |nodes| nodes[0].authenticated == 3);
    let result = a.request(&endpoint, &connection, requests::Plan::Balance);
    assert!(matches!(
        complete(&mut [&mut a], &result).result,
        Ok(requests::Reply::Ledger(
            peer::ledger_control::query::Response::Balance(91)
        ))
    ));
    let result = a.request(
        &endpoint,
        &connection,
        requests::Plan::TransferGet {
            owner: a_id,
            id: [71; 16],
        },
    );
    let reply = complete(&mut [&mut a], &result);
    let Ok(requests::Reply::Ledger(peer::ledger_control::query::Response::Transfer(proof))) =
        reply.result
    else {
        panic!("retained transfer")
    };
    assert_eq!(proof.result().received(), Ok(7));
    let result = a.request(
        &endpoint,
        &connection,
        requests::Plan::Transfer {
            signed: Box::new(replay),
            consent: Arc::new(requests::ExactFee(999)),
        },
    );
    let reply = complete(&mut [&mut a], &result);
    let Ok(requests::Reply::Transfer(original)) = reply.result else {
        panic!("signed restart replay")
    };
    assert_eq!(original.accepted_fee(), None);
    assert_eq!(original.proof().result().received(), Ok(7));
    let result = a.request(&endpoint, &connection, requests::Plan::Balance);
    assert!(matches!(
        complete(&mut [&mut a], &result).result,
        Ok(requests::Reply::Ledger(
            peer::ledger_control::query::Response::Balance(91)
        ))
    ));
    c.stop();
    b.stop();
}
