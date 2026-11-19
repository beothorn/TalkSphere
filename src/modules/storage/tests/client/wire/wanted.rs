//! Wanted Storage publisher integration retains application-owned input, consent and finite issuance; no actual Storage provider is installed.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use wire::wanted::{Policy, PublisherContexts, Work};

struct Publisher {
    endpoint: Endpoint,
    board: board::state::Provider,
    authority: Arc<board::authority::Authority>,
}
struct Application {
    input: Value,
    requester: IdentityId,
    provider: IdentityId,
    refuse: bool,
    calls: AtomicUsize,
}
impl Policy for Application {
    // A trusted application chooses the original bytes and finite spending; the incoming invitation supplies neither.
    fn choose(&self, selection: &Selection, provider: IdentityId) -> Result<Work, wire::Error> {
        assert_eq!(
            selection.roles(),
            (self.requester, self.provider, self.requester)
        );
        assert_eq!(provider, self.provider);
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.refuse {
            return Err(wire::Error::Denied);
        }
        Ok(Work {
            input: self.input.clone(),
            spending: Spending {
                maximum: 17,
                payment: Payment::default(),
            },
        })
    }
}
// Publish an actual local wanted Board with requester-only codecs; selecting it creates no remote provider installation or spending grant.
fn publisher(inv: &Invocation) -> Publisher {
    let f = fixture();
    let mut endpoint = endpoint_with_payment(f.client, inv, inv.requester, Payment::default());
    let authority = Arc::new(board::authority::Authority::new());
    let board = board::state::Provider::new(
        inv.requester,
        authority.clone(),
        BoardLimits::default(),
        endpoint.registry.clone(),
    )
    .unwrap();
    let mut offering = inv.offering.source.clone();
    offering["creditIssuer"] = crate::modules::identity::text::format(inv.requester).into();
    let raw = json!({"version":1,"providedServices":[],"wantedServices":[offering]});
    let prepared = board::prepared::Prepared::new(
        inv.requester,
        &serde_json::to_vec(&raw).unwrap(),
        &BTreeMap::from([(
            inv.offering.id.clone(),
            inv.definition.document.bytes().to_vec(),
        )]),
        BoardLimits::default(),
        endpoint.registry.as_ref(),
    )
    .unwrap();
    let publish = authority
        .grant(inv.requester, board::authority::Action::Publish, 16_777_216)
        .unwrap();
    let context = board::contract::Context {
        grant: &publish,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    };
    let snapshot = board.publish(&context, prepared).unwrap();
    let select = authority
        .grant(inv.requester, board::authority::Action::Select, 16_777_216)
        .unwrap();
    endpoint.selection = board
        .select(
            &board::contract::Context {
                grant: &select,
                ..context
            },
            &snapshot,
            &board::selection::Request {
                id: &inv.offering.id,
                service: "cr2se.storage",
                version: 1,
                description: &inv.offering.description,
                other: inv.provider,
                authenticated: true,
            },
        )
        .unwrap();
    Publisher {
        endpoint,
        board,
        authority,
    }
}
// Literal remote invitation is independent of provider encoding; unknown optional peer suggestions never become authorized application input.
fn invitation(inv: &Invocation) -> Message {
    Message::parse(&serde_json::to_vec(&json!({"type":"offer","offeringId":inv.offering.id,"service":"cr2se.storage","serviceVersion":1,"input":"REMOTE BYTES","maximumPrice":"99999","install":"https://invalid.example/handler"})).unwrap(), 65536).unwrap()
}
// Run incoming observation and outgoing policy under separate grants, rechecking the actual local Board before and after application choice.
fn request(
    publisher: &Publisher,
    association: Association,
    message: &Message,
    policy: &dyn Policy,
) -> Result<wire::requester::Request, wire::Error> {
    let e = &publisher.endpoint;
    let invoke = e
        .authority
        .grant(
            e.association.local,
            wire::authority::Action::Invoke,
            16_777_216,
        )
        .unwrap();
    let request = e
        .authority
        .grant(
            e.association.local,
            wire::authority::Action::Request,
            16_777_216,
        )
        .unwrap();
    let service = e
        .services
        .grant(
            e.association.local,
            services::authority::Action::Request,
            16_777_216,
        )
        .unwrap();
    let select = publisher
        .authority
        .grant(
            e.association.local,
            board::authority::Action::Select,
            16_777_216,
        )
        .unwrap();
    let context = board::contract::Context {
        grant: &select,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    };
    let current = services::quote::BoardCurrent {
        owner: &publisher.board,
        context: &context,
    };
    wire::wanted::request(
        &e.client,
        PublisherContexts {
            incoming: &pc(&invoke),
            request: &pc(&request),
            services: &sc(&service),
            current: &current,
        },
        association,
        e.selection.clone(),
        message,
        policy,
    )
}

// A provider-opened invitation leaves input, quote consent and finite issuance with the wanted publisher/requester.
#[test]
fn wanted_store_preserves_publisher_input_and_requester_consent() {
    let (mut inv, input, output) = store_fixture(true);
    inv.key.stream = 2;
    let publisher = publisher(&inv);
    let e = &publisher.endpoint;
    let app = Application {
        input: input.clone(),
        requester: inv.requester,
        provider: inv.provider,
        refuse: false,
        calls: AtomicUsize::new(0),
    };
    let offer = invitation(&inv);
    let request = request(&publisher, e.association, &offer, &app).unwrap();
    let proposed = Message::parse(request.bytes(), 65536).unwrap();
    assert_eq!(
        proposed.value()["publisherId"],
        crate::modules::identity::text::format(inv.requester)
    );
    assert_eq!(
        proposed.value()["creditIssuer"],
        crate::modules::identity::text::format(inv.requester)
    );
    assert_eq!(proposed.value()["maximumPrice"], "17");
    assert_eq!(
        proposed.value()["input"],
        wire::value::encode(&input, WireLimits::default(), false)
            .unwrap()
            .metadata
    );
    assert_eq!(app.calls.load(Ordering::SeqCst), 1);
    assert_eq!(e.available(), CREDIT);
    assert_eq!(e.phase(), None);
    let quote = e.quote(17, json!({}));
    let candidate = e.candidate(request, &quote).unwrap();
    assert_eq!(e.available(), CREDIT);
    let select = publisher
        .authority
        .grant(inv.requester, board::authority::Action::Select, 16_777_216)
        .unwrap();
    let context = board::contract::Context {
        grant: &select,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    };
    let current = services::quote::BoardCurrent {
        owner: &publisher.board,
        context: &context,
    };
    let outgoing = e.reserve_candidate(candidate, &current);
    assert_eq!(outgoing.input(), &input);
    assert_eq!(outgoing.invocation().requester, inv.requester);
    assert_eq!(outgoing.invocation().provider, inv.provider);
    assert_eq!(outgoing.invocation().publisher, inv.requester);
    assert_eq!(outgoing.invocation().issuer, inv.requester);
    assert_eq!(e.available(), CREDIT - 17);
    let (grant, mut receiver) = result::receiver(e, &outgoing);
    result::deliver(e, &grant, &mut receiver, &output, 17, true);
    let coordinate = e
        .services
        .grant(
            inv.requester,
            services::authority::Action::Coordinate,
            16_777_216,
        )
        .unwrap();
    receiver
        .finish(
            &pc(&grant),
            e.association,
            true,
            &sc(&coordinate),
            &e.runtime,
        )
        .unwrap();
    e.runtime.prepare(&sc(&coordinate), inv.key).unwrap();
    e.runtime
        .decide(
            &sc(&coordinate),
            inv.key,
            services::runtime::Decision::Commit,
        )
        .unwrap();
    e.runtime.commit(&sc(&coordinate), inv.key).unwrap();
    e.runtime.commit(&sc(&coordinate), inv.key).unwrap();
    let read = e
        .ledger_authority
        .grant(inv.requester, ledger::authority::Action::Read, 65536)
        .unwrap();
    assert_eq!(e.ledger.view(&lc(&read), inv.provider).unwrap().issued, 17);
    assert_eq!(e.available(), CREDIT - 17);
    // The real ordering owner agrees that the remote opener provides while the local publisher requests and accepts.
    use wire::{
        exchange::{Direction as D, Exchange, Outcome},
        message::Kind as K,
    };
    let mut exchange = Exchange::new(D::Remote);
    for (direction, kind) in [
        (D::Remote, K::Offer),
        (D::Local, K::Propose),
        (D::Remote, K::Quote),
        (D::Local, K::Accept),
        (D::Local, K::InputEnd),
    ] {
        exchange.control(direction, kind).unwrap();
    }
    exchange.end(D::Local).unwrap();
    exchange.control(D::Remote, K::Result).unwrap();
    exchange.control(D::Remote, K::ResultEnd).unwrap();
    exchange.end(D::Remote).unwrap();
    assert_eq!(exchange.close(true).unwrap(), Outcome::Complete);
}

// Wrong authenticated roles, changed invitation IDs, explicit policy refusal and withdrawn local terms cannot reserve or charge work.
#[test]
fn wanted_invitation_refusal_and_withdrawal_leave_issuance_untouched() {
    let (inv, input, _) = store_fixture(true);
    let publisher = publisher(&inv);
    let e = &publisher.endpoint;
    let mut app = Application {
        input,
        requester: inv.requester,
        provider: inv.provider,
        refuse: false,
        calls: AtomicUsize::new(0),
    };
    let offer = invitation(&inv);
    let mut wrong = e.association;
    wrong.peer = IdentityId([22; 32]);
    assert!(request(&publisher, wrong, &offer, &app).is_err());
    let mut wrong = e.association;
    wrong.operation.requester = inv.provider;
    assert!(request(&publisher, wrong, &offer, &app).is_err());
    let mut wrong = offer.value().clone();
    wrong["offeringId"] = json!("foreign");
    assert!(
        request(
            &publisher,
            e.association,
            &Message::parse(&serde_json::to_vec(&wrong).unwrap(), 65536).unwrap(),
            &app
        )
        .is_err()
    );
    assert_eq!(app.calls.load(Ordering::SeqCst), 0);
    app.refuse = true;
    assert!(request(&publisher, e.association, &offer, &app).is_err());
    assert_eq!(app.calls.load(Ordering::SeqCst), 1);
    app.refuse = false;
    withdraw(&publisher);
    assert!(request(&publisher, e.association, &offer, &app).is_err());
    assert_eq!(app.calls.load(Ordering::SeqCst), 1);
    assert_eq!(e.available(), CREDIT);
    assert_eq!(e.phase(), None);
}

// Removing the authoritative wanted publication is a real local Board transition, independent of captured invitation bytes.
fn withdraw(publisher: &Publisher) {
    let e = &publisher.endpoint;
    let grant = publisher
        .authority
        .grant(
            e.association.local,
            board::authority::Action::Publish,
            16_777_216,
        )
        .unwrap();
    let empty = board::prepared::Prepared::new(
        e.association.local,
        br#"{"version":1,"providedServices":[],"wantedServices":[]}"#,
        &BTreeMap::new(),
        BoardLimits::default(),
        e.registry.as_ref(),
    )
    .unwrap();
    publisher
        .board
        .publish(
            &board::contract::Context {
                grant: &grant,
                deadline: Instant::now() + Duration::from_secs(60),
                cancelled: false,
            },
            empty,
        )
        .unwrap();
}

struct Withdrawing<'a> {
    publisher: &'a Publisher,
    app: Application,
}
impl Policy for Withdrawing<'_> {
    // Simulate a local publication change during application work; the post-policy currentness gate must reject its stale input.
    fn choose(&self, selection: &Selection, provider: IdentityId) -> Result<Work, wire::Error> {
        let work = self.app.choose(selection, provider)?;
        withdraw(self.publisher);
        Ok(work)
    }
}
// Even locally approved input cannot become a proposal when its exact wanted terms disappear inside the policy callback.
#[test]
fn wanted_terms_are_rechecked_after_application_policy() {
    let (inv, input, _) = store_fixture(true);
    let publisher = publisher(&inv);
    let e = &publisher.endpoint;
    let policy = Withdrawing {
        publisher: &publisher,
        app: Application {
            input,
            requester: inv.requester,
            provider: inv.provider,
            refuse: false,
            calls: AtomicUsize::new(0),
        },
    };
    assert!(request(&publisher, e.association, &invitation(&inv), &policy).is_err());
    assert_eq!(policy.app.calls.load(Ordering::SeqCst), 1);
    assert_eq!(e.available(), CREDIT);
    assert_eq!(e.phase(), None);
}
