//! Real provider-opened Storage contribution with finite requester issuance and spend-back.
use super::*;
use std::sync::mpsc;
use talksphere::modules::{board, ledger};

struct Work {
    provider: IdentityId,
    input: Value,
}
impl peer::wanted::Policy for Work {
    // Only the trusted local publisher supplies input and finite spending for this provider.
    fn choose(
        &self,
        selection: &board::selection::Selection,
        provider: IdentityId,
    ) -> Result<peer::wanted::Work, peer::Error> {
        if provider != self.provider || selection.offering().id != "storage-wanted" {
            return Err(peer::Error::Denied);
        }
        Ok(peer::wanted::Work {
            input: self.input.clone(),
            spending: peer::requester::Spending {
                maximum: 17,
                payment: Default::default(),
            },
        })
    }
}

// Use known exact content and duration for both the contribution and later provider-issued spend.
fn input(duration: u64) -> Value {
    let hash = talksphere::modules::cryptography::primitives::hash::sha256(b"abc", 3).unwrap();
    fields([
        ("operation", Value::String("store".into())),
        ("sizeBytes", Value::Uint64(3)),
        ("retentionSeconds", Value::Uint64(duration)),
        ("contentHash", Value::Bytes(hash.to_vec())),
        ("data", Value::Bytes(b"abc".to_vec())),
    ])
}

// Observe independent local accounting without using authenticated balance as a write.
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

// Store over a provider-initiated wanted stream, then spend the earned issuer credits in the opposite direction.
#[test]
fn wanted_storage_contribution_earns_spendable_requester_credits() {
    run().unwrap();
}

// Keep issued and owned currency observations attached to their original authenticated roles throughout both stores.
fn run() -> Result<(), String> {
    let root = Directory::new()?;
    let ai = root.0.join("a-identity");
    let bi = root.0.join("b-identity");
    let (aid, bid) = (node::identity(&ai)?, node::identity(&bi)?);
    let (sender, receiver) = mpsc::sync_channel(1);
    let catalog = Arc::new(Catalog(Mutex::new(None)));
    let a_owner = owners::open_with_wanted(
        &root.0.join("a"),
        aid,
        bid,
        false,
        true,
        catalog.clone(),
        Some(peer::engine::endpoint::wanted::Publisher {
            policy: Arc::new(Work {
                provider: bid,
                input: input(10),
            }),
            consent: Arc::new(Ceiling(17)),
            completed: sender,
        }),
    )?;
    let b_owner = owners::open(&root.0.join("b"), bid, aid, false, true, catalog)?;
    assert_eq!(balance(&b_owner, bid, aid).owned, 0);
    assert_eq!(balance(&a_owner, aid, bid).issued, 0);
    let signal = Signal::default();
    let mut a = Host::Local(Box::new(Node::new(
        &ai,
        a_owner.endpoint.clone(),
        signal.clone(),
    )?));
    let mut b = Node::new(&bi, b_owner.endpoint.clone(), signal)?;
    let (connection, cache) = connect(&mut b, &mut a, &b_owner, aid)?;
    let pending = b.request(
        &b_owner.endpoint,
        &connection,
        Plan::Definition {
            cache: cache.clone(),
            offering: "storage-wanted".into(),
        },
    )?;
    a.complete(&mut b, &pending)?.result.map_err(failure)?;
    let selection = cache
        .selection(aid, "storage-wanted", "cr2se.storage", 1)
        .map_err(failure)?;
    assert_eq!(selection.roles(), (aid, bid, aid));
    let plan = Plan::Offer(Box::new(peer::engine::endpoint::wanted::Invitation {
        selection,
        cache: cache.clone(),
    }));
    let pending = b.request(&b_owner.endpoint, &connection, plan)?;
    let provider = a.complete(&mut b, &pending)?.result.map_err(failure)?;
    let requester = a.complete(&mut b, &receiver)?.result.map_err(failure)?;
    assert!(matches!(provider, Reply::Work(key) if key.requester == aid));
    assert!(matches!(requester, Reply::Work(key) if key.requester == aid));
    assert_eq!(balance(&b_owner, bid, aid).owned, 17);
    assert_eq!(balance(&a_owner, aid, bid).issued, 17);
    let mut spend = Network {
        requester: &mut b,
        provider: &mut a,
        owner: &b_owner,
        connection,
        cache,
        provider_id: aid,
        requester_id: bid,
    };
    let key = spend.invoke(input(1), 3)?;
    assert_eq!(spend.result(key)?.price, 3);
    assert_eq!(balance(&b_owner, bid, aid).owned, 14);
    assert_eq!(balance(&a_owner, aid, bid).issued, 14);
    a.stop()
}
