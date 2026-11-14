//! Three actual authenticated nodes separate committed work from original-issuer transfer evidence and preserve interrupted outcomes.
use super::*;
use std::sync::mpsc;
struct RecordScope {
    inner: Arc<dyn Factory>,
    scopes: mpsc::SyncSender<Scope>,
}
impl Factory for RecordScope {
    // Observe only scopes of successfully constructed real handlers; this fixture never supplies authentication or financial capabilities.
    fn open(
        &self,
        scope: Scope,
        opener: Direction,
        output: &mut Output,
    ) -> Result<Box<dyn Handler>, Error> {
        let handler = self.inner.open(scope, opener, output)?;
        self.scopes.try_send(scope).map_err(|_| Error::Limit)?;
        Ok(handler)
    }
}
// Explicitly republish the installed custom contract with the independently chosen third issuer.
fn publish_third_party(world: &World, issuer: IdentityId) {
    let offering = world.family.offering(issuer);
    let definition = services::handler::Requester::definition(world.family.as_ref());
    let raw =
        serde_json::json!({"version":1,"providedServices":[offering.source],"wantedServices":[]})
            .to_string();
    let prepared = board::prepared::Prepared::new(
        world.local,
        raw.as_bytes(),
        &std::collections::BTreeMap::from([(offering.id, definition.document.bytes().to_vec())]),
        Default::default(),
        world.registry.as_ref(),
    )
    .unwrap();
    let publish = world
        .board_authority
        .grant(world.local, board::authority::Action::Publish, 16_777_216)
        .unwrap();
    world
        .board
        .publish(
            &board_context(&publish, Instant::now() + Duration::from_secs(30)),
            prepared,
        )
        .unwrap();
}
// Only opaque normally completed independent-issuer observations settle an already committed original local book.
fn confirm(world: &World, original: Association, proof: &peer::ledger_control::query::Transfer) {
    let pg = world
        .authority
        .grant(world.local, Action::Coordinate, 16_777_216)
        .unwrap();
    let evidence = world
        .ledger_authority
        .grant(world.local, ledger::authority::Action::Evidence, 16_777_216)
        .unwrap();
    let coordinate = world
        .ledger_authority
        .grant(
            world.local,
            ledger::authority::Action::Coordinate,
            16_777_216,
        )
        .unwrap();
    let ec = peer::test_support::world::ledger_context(&evidence);
    let cc = peer::test_support::world::ledger_context(&coordinate);
    for _ in 0..2 {
        peer::ledger_control::settlement::confirm(
            &context(&pg),
            &world.authority,
            original,
            proof,
            peer::ledger_control::settlement::Ports {
                owner: &world.ledger,
                authority: world.ledger_authority.clone(),
                evidence: &ec,
                coordinate: &cc,
            },
        )
        .unwrap();
    }
    assert_eq!(
        world.ledger.payment(
            &cc,
            ledger::contract::OperationKey {
                requester: original.operation.requester,
                connection: original.operation.connection,
                stream: original.operation.stream
            }
        ),
        Ok(Some(ledger::payment::Payment::Settled))
    );
}
// Successful third-party work commits before payment; each owner then independently observes the real issuer and settles once.
#[test]
fn real_nodes_settle_third_party_work_and_preserve_interrupted_control_uncertainty() {
    let root = Directory::new();
    let a_id = identity(&root.0.join("a"));
    let b_id = identity(&root.0.join("b"));
    let c_id = identity(&root.0.join("c"));
    let order = ledger::transfer::Order {
        issuer: c_id,
        owner: a_id,
        new_owner: b_id,
        transfer_id: [83; 16],
        amount: 9,
    };
    let signed = processes::signed(&root.0.join("a"), order);
    let policy = ledger::terms::Policy {
        third_party: true,
        ..Default::default()
    };
    let aw = World::with_policy(a_id, b_id, true, Arc::new(Echo::new(false)), policy);
    let bw = World::with_policy(b_id, a_id, false, Arc::new(Echo::new(false)), policy);
    let cw = World::with_identities(c_id, a_id, false, Arc::new(Echo::new(false)));
    let admin = aw
        .ledger_authority
        .grant(a_id, ledger::authority::Action::Administer, 16_777_216)
        .unwrap();
    aw.ledger
        .set_owned(
            &peer::test_support::world::ledger_context(&admin),
            c_id,
            100,
        )
        .unwrap();
    publish_third_party(&bw, c_id);
    let ae = endpoint_world(&aw);
    let be = endpoint_world(&bw);
    let signal = Signal::default();
    let (scopes, provider_scopes) = mpsc::sync_channel(32);
    let mut a = Node::new(&root.0.join("a"), Arc::new(ae.clone()), signal.clone());
    let mut b = Node::new(
        &root.0.join("b"),
        Arc::new(RecordScope {
            inner: Arc::new(be.clone()),
            scopes,
        }),
        signal.clone(),
    );
    let mut c = Node::new(
        &root.0.join("c"),
        processes::issuer_endpoint(&cw, &root.0.join("issuer-ledger")),
        signal,
    );
    let ab = a.connect(b.address, b_id);
    let ac = a.connect(c.address, c_id);
    let bc = b.connect(c.address, c_id);
    until(&mut [&mut a, &mut b, &mut c], |nodes| {
        nodes.iter().all(|n| n.authenticated == 2)
    });
    let cache = Arc::new(cache::PeerCache::new(aw.registry.clone(), Default::default()).unwrap());
    for plan in [
        requests::Plan::Board(cache.clone()),
        requests::Plan::Definition {
            cache: cache.clone(),
            offering: "fixture".into(),
        },
    ] {
        let result = a.request(&ae, &ab, plan);
        assert!(
            complete(&mut [&mut a, &mut b, &mut c], &result)
                .result
                .is_ok()
        );
    }
    let o = bw.family.offering(c_id);
    let selection = cache.selection(b_id, &o.id, &o.service, o.version).unwrap();
    let result = a.request(
        &ae,
        &ab,
        requests::Plan::Invoke(Box::new(requests::Invocation {
            selection,
            cache,
            input: Value::String("committed third-party work".into()),
            spending: peer::requester::Spending {
                maximum: 9,
                payment: services::runtime::Payment {
                    transfer: Some([83; 16]),
                    fee: Some(2),
                },
            },
            consent: Arc::new(requests::Ceiling(9)),
        })),
    );
    let reply = complete(&mut [&mut a, &mut b, &mut c], &result);
    assert!(
        matches!(reply.result, Ok(requests::Reply::Work(_))),
        "third-party work refused: {:?}",
        reply.cause
    );
    let original_a = reply.scope.association(a_id).unwrap();
    let original_b = provider_scopes
        .try_iter()
        .find(|s| s.stream == reply.scope.stream)
        .unwrap()
        .association(a_id)
        .unwrap();
    assert_ne!(
        original_a.operation.connection,
        original_b.operation.connection
    );
    let result = a.request(
        &ae,
        &ac,
        requests::Plan::Transfer {
            signed: Box::new(signed),
            consent: Arc::new(requests::ExactFee(2)),
        },
    );
    assert!(matches!(
        complete(&mut [&mut a, &mut b, &mut c], &result).result,
        Ok(requests::Reply::Transfer(_))
    ));
    for (requester, connection, world, original) in
        [(true, &ac, &aw, original_a), (false, &bc, &bw, original_b)]
    {
        let result = if requester {
            a.request(
                &ae,
                connection,
                requests::Plan::TransferGet {
                    owner: a_id,
                    id: [83; 16],
                },
            )
        } else {
            b.request(
                &be,
                connection,
                requests::Plan::TransferGet {
                    owner: a_id,
                    id: [83; 16],
                },
            )
        };
        let reply = complete(&mut [&mut a, &mut b, &mut c], &result);
        let Ok(requests::Reply::Ledger(peer::ledger_control::query::Response::Transfer(proof))) =
            reply.result
        else {
            panic!("independent issuer observation")
        };
        confirm(world, original, &proof);
    }
    let read = aw
        .ledger_authority
        .grant(a_id, ledger::authority::Action::Read, 16_777_216)
        .unwrap();
    assert_eq!(
        aw.ledger
            .view(&peer::test_support::world::ledger_context(&read), c_id)
            .unwrap()
            .owned,
        91
    );
    let read = bw
        .ledger_authority
        .grant(b_id, ledger::authority::Action::Read, 16_777_216)
        .unwrap();
    assert_eq!(
        bw.ledger
            .view(&peer::test_support::world::ledger_context(&read), c_id)
            .unwrap()
            .owned,
        7
    );
    assert_eq!(bw.family.executions.load(Ordering::SeqCst), 1);
    // Close a constructed control stream before polling its remote node; no provisional response can become success.
    let (call, result) = ae.request(requests::Plan::Balance);
    let (opened, ready) = mpsc::sync_channel(1);
    a.bridge
        .open_with(
            &mut a.transport.driver,
            &ab,
            Arc::new(RecordScope {
                inner: Arc::new(call),
                scopes: opened,
            }),
        )
        .unwrap();
    until(&mut [&mut a], |_| ready.try_recv().is_ok());
    a.transport
        .driver
        .submit(Request::Close { connection: ab })
        .unwrap();
    assert!(matches!(
        complete(&mut [&mut a, &mut b, &mut c], &result).result,
        Err(Error::Unknown)
    ));
    let result = a.request(&ae, &ac, requests::Plan::Balance);
    assert!(matches!(
        complete(&mut [&mut a, &mut b, &mut c], &result).result,
        Ok(requests::Reply::Ledger(
            peer::ledger_control::query::Response::Balance(91)
        ))
    ));
}
