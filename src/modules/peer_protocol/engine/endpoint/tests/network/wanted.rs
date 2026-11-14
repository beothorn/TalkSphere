//! Actual authenticated node exchanges retain wanted roles, local input authority and complete included evidence consumption.
use super::*;
struct LocalWork;
impl peer::wanted::Policy for LocalWork {
    // Only this installed local policy chooses publisher input/spending, independently of the provider's invitation.
    fn choose(
        &self,
        _: &board::selection::Selection,
        _: IdentityId,
    ) -> Result<peer::wanted::Work, Error> {
        Ok(peer::wanted::Work {
            input: Value::String("locally chosen wanted body 🦀".repeat(500)),
            spending: peer::requester::Spending {
                maximum: 7,
                payment: Default::default(),
            },
        })
    }
}
// Two actual TCP endpoints complete reversed-role detached work and one included check without conflating the stream opener with the payer.
#[test]
fn authenticated_wanted_bodies_and_included_evidence_complete_between_nodes() {
    let root = Directory::new();
    let a_id = identity(&root.0.join("a"));
    let b_id = identity(&root.0.join("b"));
    let a_world = World::with_identities(a_id, b_id, true, Arc::new(Echo::with_checks(true)));
    let b_world = World::with_identities(b_id, a_id, false, Arc::new(Echo::with_checks(true)));
    let o = a_world.family.offering(b_id);
    let definition = services::handler::Requester::definition(a_world.family.as_ref());
    let raw = serde_json::json!({"version":1,"providedServices":[],"wantedServices":[o.source]})
        .to_string();
    let prepared = board::prepared::Prepared::new(
        a_id,
        raw.as_bytes(),
        &std::collections::BTreeMap::from([(o.id.clone(), definition.document.bytes().to_vec())]),
        Default::default(),
        a_world.registry.as_ref(),
    )
    .unwrap();
    let publish = a_world
        .board_authority
        .grant(a_id, board::authority::Action::Publish, 16_777_216)
        .unwrap();
    a_world
        .board
        .publish(
            &board_context(&publish, Instant::now() + Duration::from_secs(30)),
            prepared,
        )
        .unwrap();
    let (completed, publisher_results) = std::sync::mpsc::sync_channel(1);
    let mut config = endpoint_config(&a_world);
    config.wanted = Some(wanted::Publisher {
        policy: Arc::new(LocalWork),
        consent: Arc::new(requests::Ceiling(7)),
        completed,
    });
    let a_endpoint = Endpoint::new(config).unwrap();
    let b_endpoint = endpoint_world(&b_world);
    let signal = Signal::default();
    let mut a = Node::new(
        &root.0.join("a"),
        Arc::new(a_endpoint.clone()),
        signal.clone(),
    );
    let mut b = Node::new(&root.0.join("b"), Arc::new(b_endpoint.clone()), signal);
    let connection = b.connect(a.address, a_id);
    until(&mut [&mut a, &mut b], |nodes| {
        nodes.iter().all(|node| node.authenticated == 1)
    });
    let cache =
        Arc::new(cache::PeerCache::new(b_world.registry.clone(), Default::default()).unwrap());
    for plan in [
        requests::Plan::Board(cache.clone()),
        requests::Plan::Definition {
            cache: cache.clone(),
            offering: o.id.clone(),
        },
    ] {
        let result = b.request(&b_endpoint, &connection, plan);
        assert!(complete(&mut [&mut a, &mut b], &result).result.is_ok());
    }
    let selection = cache.selection(a_id, &o.id, &o.service, o.version).unwrap();
    let result = b.request(
        &b_endpoint,
        &connection,
        requests::Plan::Offer(Box::new(wanted::Invitation { selection, cache })),
    );
    let provider = complete(&mut [&mut a, &mut b], &result);
    assert!(matches!(provider.result,Ok(requests::Reply::Work(key)) if key.requester==a_id));
    let requester = complete(&mut [&mut a, &mut b], &publisher_results);
    assert!(
        matches!(requester.result,Ok(requests::Reply::Work(key)) if key.requester==a_id),
        "publisher failed: {:?}, cause {:?}, remote {:?}",
        requester.result.as_ref().err(),
        requester.cause,
        requester.failure
    );
    assert_eq!(b_world.family.executions.load(Ordering::SeqCst), 1);
    // Provided metadata on the same authenticated connection remains independent of the preceding wanted roles.
    let connection = a.connect(b.address, b_id);
    until(&mut [&mut a, &mut b], |nodes| {
        nodes.iter().all(|node| node.authenticated == 2)
    });
    let cache =
        Arc::new(cache::PeerCache::new(a_world.registry.clone(), Default::default()).unwrap());
    for plan in [
        requests::Plan::Board(cache.clone()),
        requests::Plan::Definition {
            cache: cache.clone(),
            offering: o.id.clone(),
        },
    ] {
        let result = a.request(&a_endpoint, &connection, plan);
        assert!(complete(&mut [&mut a, &mut b], &result).result.is_ok());
    }
    let selection = cache.selection(b_id, &o.id, &o.service, o.version).unwrap();
    let result = a.request(
        &a_endpoint,
        &connection,
        requests::Plan::Check(Box::new(requests::Challenge {
            selection,
            cache,
            challenge: Value::Uint64(1),
            spending: peer::requester::Spending {
                maximum: 0,
                payment: Default::default(),
            },
            consent: Arc::new(requests::Ceiling(0)),
        })),
    );
    assert!(matches!(
        complete(&mut [&mut a, &mut b], &result).result,
        Ok(requests::Reply::Check(_))
    ));
    assert_eq!(b_world.family.evidence_calls.load(Ordering::SeqCst), 1);
}
