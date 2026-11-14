//! Wanted invitations preserve locally chosen input, spending and direction-independent accounting roles.
use super::*;
struct WantedWork {
    calls: std::sync::atomic::AtomicUsize,
}
impl peer::wanted::Policy for WantedWork {
    // A single locally owned work item is authorized; another remote invitation cannot invent another spending intent.
    fn choose(
        &self,
        _: &board::selection::Selection,
        _: IdentityId,
    ) -> Result<peer::wanted::Work, Error> {
        if self.calls.fetch_add(1, Ordering::SeqCst) != 0 {
            return Err(Error::Denied);
        }
        Ok(peer::wanted::Work {
            input: Value::String("LOCAL_INPUT".into()),
            spending: peer::requester::Spending {
                maximum: 7,
                payment: Default::default(),
            },
        })
    }
}
// Wanted publishers retain application input/spending and requester accounting even when the provider opens the stream.
#[test]
fn explicit_wanted_invitation_reverses_roles_and_commits_locally_chosen_work() {
    let pair = Pair::new(false);
    let r = &pair.requester;
    let p = &pair.provider;
    let o = r.family.offering(p.local);
    let raw = serde_json::json!({"version":1,"providedServices":[],"wantedServices":[o.source]})
        .to_string();
    let definition = services::handler::Requester::definition(r.family.as_ref());
    let prepared = board::prepared::Prepared::new(
        r.local,
        raw.as_bytes(),
        &std::collections::BTreeMap::from([(o.id.clone(), definition.document.bytes().to_vec())]),
        Default::default(),
        r.registry.as_ref(),
    )
    .unwrap();
    let grant = r
        .board_authority
        .grant(r.local, board::authority::Action::Publish, 16_777_216)
        .unwrap();
    r.board
        .publish(
            &board_context(&grant, Instant::now() + Duration::from_secs(60)),
            prepared,
        )
        .unwrap();
    let (policy_completed, publisher_results) = std::sync::mpsc::sync_channel(1);
    let policy = Arc::new(WantedWork {
        calls: std::sync::atomic::AtomicUsize::new(0),
    });
    let mut config = endpoint_config(r);
    config.wanted = Some(wanted::Publisher {
        policy: policy.clone(),
        consent: Arc::new(requests::Ceiling(7)),
        completed: policy_completed,
    });
    let publisher = Endpoint::new(config).unwrap();
    let provider = endpoint(&pair);
    let cache = Arc::new(cache::PeerCache::new(p.registry.clone(), Default::default()).unwrap());
    call(
        &provider,
        &publisher,
        1,
        requests::Plan::Board(cache.clone()),
    )
    .result
    .unwrap();
    call(
        &provider,
        &publisher,
        3,
        requests::Plan::Definition {
            cache: cache.clone(),
            offering: o.id.clone(),
        },
    )
    .result
    .unwrap();
    let selection = cache
        .selection(r.local, &o.id, &o.service, o.version)
        .unwrap();
    assert_eq!(selection.roles().0, r.local);
    assert_eq!(selection.roles().1, p.local);
    let reply = call(
        &provider,
        &publisher,
        5,
        requests::Plan::Offer(Box::new(wanted::Invitation { selection, cache })),
    );
    let Ok(requests::Reply::Work(provider_key)) = reply.result else {
        panic!("provider committed wanted work")
    };
    let reply = publisher_results.try_recv().unwrap();
    let Ok(requests::Reply::Work(requester_key)) = reply.result else {
        panic!("publisher requester committed wanted work")
    };
    assert_eq!(policy.calls.load(Ordering::SeqCst), 1);
    assert_eq!(p.family.executions.load(Ordering::SeqCst), 1);
    assert_eq!(pair.owned_available(), 93);
    for (world, key) in [(r, requester_key), (p, provider_key)] {
        let observe = world
            .services
            .grant(
                world.local,
                services::authority::Action::Observe,
                16_777_216,
            )
            .unwrap();
        assert_eq!(
            world
                .runtime
                .completed(&service_context(&observe), key)
                .unwrap()
                .1
                .output,
            Value::String("LOCAL_INPUT".into())
        );
        assert_eq!(key.requester, r.local);
    }
}

// Merely publishing wanted metadata creates no application authority; absent local policy refuses the provider invitation normally.
#[test]
fn wanted_publication_without_local_policy_does_not_authorize_input_or_spending() {
    let pair = Pair::new(false);
    let r = &pair.requester;
    let p = &pair.provider;
    let o = r.family.offering(p.local);
    let raw = serde_json::json!({"version":1,"providedServices":[],"wantedServices":[o.source]})
        .to_string();
    let definition = services::handler::Requester::definition(r.family.as_ref());
    let prepared = board::prepared::Prepared::new(
        r.local,
        raw.as_bytes(),
        &std::collections::BTreeMap::from([(o.id.clone(), definition.document.bytes().to_vec())]),
        Default::default(),
        r.registry.as_ref(),
    )
    .unwrap();
    let grant = r
        .board_authority
        .grant(r.local, board::authority::Action::Publish, 16_777_216)
        .unwrap();
    r.board
        .publish(
            &board_context(&grant, Instant::now() + Duration::from_secs(60)),
            prepared,
        )
        .unwrap();
    let provider = endpoint(&pair);
    let publisher = endpoint_world(r);
    let cache = Arc::new(cache::PeerCache::new(p.registry.clone(), Default::default()).unwrap());
    call(
        &provider,
        &publisher,
        1,
        requests::Plan::Board(cache.clone()),
    )
    .result
    .unwrap();
    call(
        &provider,
        &publisher,
        3,
        requests::Plan::Definition {
            cache: cache.clone(),
            offering: o.id.clone(),
        },
    )
    .result
    .unwrap();
    let selection = cache
        .selection(r.local, &o.id, &o.service, o.version)
        .unwrap();
    let reply = call(
        &provider,
        &publisher,
        5,
        requests::Plan::Offer(Box::new(wanted::Invitation { selection, cache })),
    );
    assert!(matches!(reply.result, Err(Error::Unsupported)));
    assert_eq!(
        reply.failure.unwrap().code,
        peer::failure::Code::Unsupported
    );
    assert_eq!(pair.owned_available(), 100);
    assert_eq!(p.family.executions.load(Ordering::SeqCst), 0);
}
