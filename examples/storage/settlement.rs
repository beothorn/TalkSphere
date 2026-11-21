//! Three authenticated nodes exercise actual Storage with independent issuer settlement.
use super::*;
use std::{path::Path, sync::mpsc};
use talksphere::modules::{
    cryptography::random::SystemEntropy,
    identity::{self, api::IdentityProvider, authority::Sign},
    ledger,
    peer_protocol::{
        contract::{Association, Context},
        engine::bridge::{Factory, Handler, Output, Scope},
        exchange::Direction,
        ledger_control::{query, settlement, transfer_client::Signed},
    },
};

struct Observe {
    actual: owners::Endpoint,
    scopes: mpsc::SyncSender<Scope>,
}
impl Factory for Observe {
    // Observe an actually authenticated incoming stream while forwarding all handling to the real provider.
    fn open(
        &self,
        scope: Scope,
        opener: Direction,
        output: &mut Output,
    ) -> Result<Box<dyn Handler>, peer::Error> {
        let result = self.actual.open(scope, opener, output)?;
        self.scopes
            .try_send(scope)
            .map_err(|_| peer::Error::Limit)?;
        Ok(result)
    }
}

// Sign the exact original order with a scoped Identity handle before the node acquires the exclusive key store.
fn sign(path: &Path, order: ledger::transfer::Order) -> Result<Signed, String> {
    let limits = identity::config::Limits::new(256, 8, 4096, 8192, 4096).map_err(failure)?;
    let (store, admin) = identity::storage::FileStore::open(path, limits).map_err(failure)?;
    let mut identity =
        identity::provider::Identity::new(store, SystemEntropy, limits, 1).map_err(failure)?;
    let (handle, public) = identity.load(&admin).map_err(failure)?;
    let grant = identity
        .grant::<Sign>(&admin, &handle, 121)
        .map_err(failure)?;
    let signature = identity
        .sign(&handle, &grant, &order.encode().map_err(failure)?)
        .map_err(failure)?;
    Ok(Signed {
        order,
        key: public.identity.signing_public_key(),
        signature,
        maximum_fee: 2,
    })
}

// Observe one exact issuer-specific balance at the independently configured book.
fn balance(owner: &Owners, local: IdentityId, other: IdentityId) -> ledger::accounting::View {
    let grant = owner
        .ledger_authority
        .grant(local, ledger::authority::Action::Read, 16_777_216)
        .unwrap();
    owner
        .account
        .view(
            &ledger::contract::Context {
                version: 1,
                grant: &grant,
                deadline: Instant::now() + Duration::from_secs(10),
                cancelled: false,
            },
            other,
        )
        .unwrap()
}

// Accept only an independent completed issuer observation for this exact retained invocation.
fn settle(owner: &Owners, original: Association, proof: &query::Transfer) -> Result<(), String> {
    let local = original.local;
    let grant = owner
        .peer_authority
        .grant(local, peer::authority::Action::Coordinate, 16_777_216)
        .map_err(failure)?;
    let evidence = owner
        .ledger_authority
        .grant(local, ledger::authority::Action::Evidence, 16_777_216)
        .map_err(failure)?;
    let coordinate = owner
        .ledger_authority
        .grant(local, ledger::authority::Action::Coordinate, 16_777_216)
        .map_err(failure)?;
    let deadline = Instant::now() + Duration::from_secs(10);
    let ec = ledger::contract::Context {
        version: 1,
        grant: &evidence,
        deadline,
        cancelled: false,
    };
    let cc = ledger::contract::Context {
        grant: &coordinate,
        ..ec
    };
    let pc = Context {
        version: 1,
        grant: &grant,
        deadline,
        cancelled: false,
    };
    for _ in 0..2 {
        settlement::confirm(
            &pc,
            &owner.peer_authority,
            original,
            proof,
            settlement::Ports {
                owner: &owner.account,
                authority: owner.ledger_authority.clone(),
                evidence: &ec,
                coordinate: &cc,
            },
        )
        .map_err(failure)?;
    }
    Ok(())
}

// Exact third-party fee consent and original-issuer provenance settle each Storage book once after completion.
#[test]
fn actual_storage_settles_after_independent_issuer_transfer() {
    run().unwrap();
}

// Compose original authenticated roles, verify the pre-consent refusal, and transfer the exact bound after successful work.
fn run() -> Result<(), String> {
    let root = Directory::new()?;
    let ai = root.0.join("ai");
    let bi = root.0.join("bi");
    let ci = root.0.join("ci");
    let (aid, bid, cid) = (
        node::identity(&ai)?,
        node::identity(&bi)?,
        node::identity(&ci)?,
    );
    let id = [12; 16];
    let signed = sign(
        &ai,
        ledger::transfer::Order {
            issuer: cid,
            owner: aid,
            new_owner: bid,
            transfer_id: id,
            amount: 19,
        },
    )?;
    let empty = || Arc::new(Catalog(Mutex::new(None)));
    let a_owner = owners::open(&root.0.join("a"), aid, cid, true, true, empty())?;
    let b_owner = owners::open_configured(
        &root.0.join("b"),
        bid,
        aid,
        false,
        true,
        empty(),
        owners::PaymentProfile {
            issuer: Some(cid),
            allow_third_party: true,
            fee: 0,
        },
        None,
    )?;
    let c_owner = owners::open_configured(
        &root.0.join("c"),
        cid,
        aid,
        true,
        true,
        empty(),
        owners::PaymentProfile {
            fee: 2,
            ..Default::default()
        },
        None,
    )?;
    let signal = Signal::default();
    let (sender, scopes) = mpsc::sync_channel(16);
    let mut a = Node::new(&ai, a_owner.endpoint.clone(), signal.clone())?;
    let mut b = Node::with_factory(
        &bi,
        Arc::new(Observe {
            actual: b_owner.endpoint.clone(),
            scopes: sender,
        }),
        signal.clone(),
    )?;
    let mut c = Node::new(&ci, c_owner.endpoint.clone(), signal)?;
    let ab = a.connect(b.address, bid)?;
    let ac = a.connect(c.address, cid)?;
    let bc = b.connect(c.address, cid)?;
    node::until(&mut [&mut a, &mut b, &mut c], |nodes| {
        nodes.iter().all(|n| n.authenticated == 2)
    })?;
    let cache =
        Arc::new(PeerCache::new(a_owner.registry.clone(), Default::default()).map_err(failure)?);
    for plan in [
        Plan::Board(cache.clone()),
        Plan::Definition {
            cache: cache.clone(),
            offering: "storage-standard".into(),
        },
    ] {
        let pending = a.request(&a_owner.endpoint, &ab, plan)?;
        node::complete(&mut [&mut a, &mut b, &mut c], &pending)?
            .result
            .map_err(failure)?;
    }
    let selection = cache
        .selection(bid, "storage-standard", "cr2se.storage", 1)
        .map_err(failure)?;
    assert_eq!(selection.roles(), (aid, bid, cid));
    let hash =
        talksphere::modules::cryptography::primitives::hash::sha256(b"abc", 3).map_err(failure)?;
    let input = fields([
        ("operation", Value::String("store".into())),
        ("sizeBytes", Value::Uint64(3)),
        ("retentionSeconds", Value::Uint64(10)),
        ("contentHash", Value::Bytes(hash.to_vec())),
        ("data", Value::Bytes(b"abc".to_vec())),
    ]);
    let plan = |maximum| {
        Plan::Invoke(Box::new(Invocation {
            selection: selection.clone(),
            cache: cache.clone(),
            input: input.clone(),
            spending: peer::requester::Spending {
                maximum,
                payment: services::runtime::Payment {
                    transfer: Some(id),
                    fee: Some(2),
                },
            },
            consent: Arc::new(Ceiling(maximum)),
        }))
    };
    let refused = a.request(&a_owner.endpoint, &ab, plan(18))?;
    assert!(
        node::complete(&mut [&mut a, &mut b, &mut c], &refused)?
            .result
            .is_err()
    );
    assert_eq!(balance(&a_owner, aid, cid).owned, 1_000_000);
    let pending = a.request(&a_owner.endpoint, &ab, plan(19))?;
    let complete = node::complete(&mut [&mut a, &mut b, &mut c], &pending)?;
    let Reply::Work(key) = complete.result.map_err(failure)? else {
        return Err("Expected committed Storage result".into());
    };
    let original_a = complete.scope.association(aid).map_err(failure)?;
    let original_b = scopes
        .try_iter()
        .find(|scope| scope.stream == complete.scope.stream)
        .ok_or("Missing authenticated provider stream")?
        .association(aid)
        .map_err(failure)?;
    assert_eq!(balance(&b_owner, bid, cid).owned, 0);
    let observe = a_owner
        .services
        .grant(aid, services::authority::Action::Observe, 16_777_216)
        .map_err(failure)?;
    let oc = services::contract::Context {
        version: 1,
        grant: &observe,
        deadline: Instant::now() + Duration::from_secs(10),
        cancelled: false,
    };
    let original_result = a_owner.runtime.result(&oc, key).map_err(failure)?;
    assert_eq!(original_result.price, 17);
    let pending = a.request(
        &a_owner.endpoint,
        &ac,
        Plan::Transfer {
            signed: Box::new(signed),
            consent: Arc::new(peer::engine::endpoint::requests::ExactFee(2)),
        },
    )?;
    assert!(matches!(
        node::complete(&mut [&mut a, &mut b, &mut c], &pending)?
            .result
            .map_err(failure)?,
        Reply::Transfer(_)
    ));
    let plan = || Plan::TransferGet { owner: aid, id };
    let request_a = a.request(&a_owner.endpoint, &ac, plan())?;
    let request_b = b.request(&b_owner.endpoint, &bc, plan())?;
    for (pending, owner, original) in [
        (request_a, &a_owner, original_a),
        (request_b, &b_owner, original_b),
    ] {
        let Reply::Ledger(query::Response::Transfer(proof)) =
            node::complete(&mut [&mut a, &mut b, &mut c], &pending)?
                .result
                .map_err(failure)?
        else {
            return Err("Expected independent issuer proof".into());
        };
        settle(owner, original, &proof)?;
    }
    assert_eq!(balance(&a_owner, aid, cid).owned, 1_000_000 - 19);
    assert_eq!(balance(&b_owner, bid, cid).owned, 17);
    assert_eq!(balance(&c_owner, cid, aid).issued, 1_000_000 - 19);
    assert_eq!(balance(&c_owner, cid, bid).issued, 17);
    assert_eq!(
        a_owner.runtime.result(&oc, key).map_err(failure)?.output,
        original_result.output
    );
    Ok(())
}
