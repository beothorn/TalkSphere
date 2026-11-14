//! Two independently owned real module stacks exercise proposal, quote and reservation through public ports.
use super::world::{World, context, service_context};
use crate::modules::{
    board,
    peer_protocol::{
        Error,
        acceptance::{Admission, Incoming, Outgoing},
        authority::Action,
        contract::{Association, Limits},
        message::Message,
        proposal::Authorizer,
        quote::{Entropy, Issued, Issuer},
        requester::{Client, Request, Spending},
    },
    services,
};
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};
struct Random(AtomicU8);
impl Entropy for Random {
    // The fixture counts deterministic fresh IDs and fails instead of wrapping to a reused value.
    fn quote_id(&self) -> Result<[u8; 32], Error> {
        self.0
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_add(1))
            .map(|n| [n; 32])
            .map_err(|_| Error::Unavailable)
    }
}
pub(in crate::modules::peer_protocol) struct Pair<F = super::family::Echo> {
    pub provider: World<F>,
    pub requester: World<F>,
    pub issuer: Issuer,
    pub client: Client,
}
impl Pair {
    // Each endpoint owns its own authority, Registry, Ledger and coordinator; only the exact custom definition is shared.
    pub fn new(bytes: bool) -> Self {
        let provider = World::new(bytes);
        let requester = World::requester(bytes);
        Self::from_worlds(provider, requester)
    }
    // Existing custom resources expose explicit paid/included evidence while retaining real independent participant owners.
    pub fn checks(included: bool) -> Self {
        Self::from_worlds(
            World::checks(false, included),
            World::checks(true, included),
        )
    }
}
impl<F: super::world::Family + 'static> Pair<F> {
    // Shared trusted setup is independent of whether remote checks were explicitly installed.
    pub(in crate::modules::peer_protocol) fn from_worlds(
        provider: World<F>,
        requester: World<F>,
    ) -> Self {
        let issuer = Issuer::new(
            provider.local,
            provider.authority.clone(),
            provider.services.clone(),
            Limits::default(),
            Default::default(),
            provider.gate.clone(),
            Arc::new(Random(AtomicU8::new(1))),
        )
        .unwrap();
        let client = Client::new(
            requester.local,
            requester.authority.clone(),
            requester.services.clone(),
            requester.registry.clone(),
            Limits::default(),
            Default::default(),
        )
        .unwrap();
        Self {
            provider,
            requester,
            issuer,
            client,
        }
    }
    // Generate actual requester proposal bytes and assess that same message at the independent provider.
    pub fn propose(
        &self,
        stream: u32,
        input: services::value::Value,
        maximum: u64,
    ) -> (Issued, Request) {
        let p = &self.provider;
        let r = &self.requester;
        let (pa, selection, _) = p.proposal(stream, &input);
        let ra = Association {
            local: r.local,
            peer: p.local,
            ..pa
        };
        let rg = r
            .authority
            .grant(r.local, Action::Request, 16_777_216)
            .unwrap();
        let rsg = r
            .services
            .grant(r.local, services::authority::Action::Request, 16_777_216)
            .unwrap();
        let request = self
            .client
            .request(
                &context(&rg),
                &service_context(&rsg),
                ra,
                selection.clone(),
                input,
                Spending {
                    maximum,
                    payment: Default::default(),
                },
            )
            .unwrap();
        let message = Message::parse(request.bytes(), 65536).unwrap();
        let pg = p
            .authority
            .grant(p.local, Action::Invoke, 16_777_216)
            .unwrap();
        let sg = p
            .services
            .grant(p.local, services::authority::Action::Invoke, 16_777_216)
            .unwrap();
        let pc = context(&pg);
        let sc = service_context(&sg);
        let authorizer = Authorizer::new(
            p.local,
            p.authority.clone(),
            p.services.clone(),
            p.registry.clone(),
            Limits::default(),
            p.gate.clone(),
        )
        .unwrap();
        let proposal = authorizer
            .authorize(&pc, &sc, pa, selection, &message)
            .unwrap();
        let bg = p
            .board_authority
            .grant(p.local, board::authority::Action::Select, 16_777_216)
            .unwrap();
        let bc = board::contract::Context {
            grant: &bg,
            deadline: pc.deadline,
            cancelled: false,
        };
        let current = services::quote::BoardCurrent {
            owner: &p.board,
            context: &bc,
        };
        (
            self.issuer
                .issue(&pc, &sc, proposal, &current, pc.deadline)
                .unwrap(),
            request,
        )
    }
    // Reserve the real requester journal/Ledger and mark one dispatch before exposing accept bytes.
    pub fn reserve(&self, issued: &Issued, request: Request) -> Result<Outgoing, Error> {
        let p = &self.provider;
        let r = &self.requester;
        let rg = r
            .authority
            .grant(r.local, Action::Request, 16_777_216)
            .unwrap();
        let ig = r
            .authority
            .grant(r.local, Action::Invoke, 16_777_216)
            .unwrap();
        let sg = r
            .services
            .grant(r.local, services::authority::Action::Request, 16_777_216)
            .unwrap();
        let cg = r
            .services
            .grant(r.local, services::authority::Action::Coordinate, 16_777_216)
            .unwrap();
        let c = context(&rg);
        let sc = service_context(&sg);
        let cc = service_context(&cg);
        let message = Message::parse(issued.bytes(), 65536).unwrap();
        let a = Association {
            local: r.local,
            peer: p.local,
            ..issued.association()
        };
        let candidate = self
            .client
            .validate(&context(&ig), &sc, request, a, &message)?;
        let hash = candidate.hash();
        let consent = self.client.consent(&c, candidate, hash, 100)?;
        let bg = p
            .board_authority
            .grant(p.local, board::authority::Action::Select, 16_777_216)
            .unwrap();
        let bc = board::contract::Context {
            grant: &bg,
            deadline: c.deadline,
            cancelled: false,
        };
        let current = services::quote::BoardCurrent {
            owner: &p.board,
            context: &bc,
        };
        self.client.reserve(
            &c,
            Admission {
                quote_context: &sc,
                coordinate_context: &cc,
                current: &current,
                runtime: r.runtime.as_ref(),
            },
            consent,
        )
    }
    // Provider reservation follows only exact incoming acceptance; invalid attempts cannot reach the real coordinator.
    pub fn accept(
        &self,
        issued: Issued,
        observed: Association,
        message: &Message,
    ) -> Result<Incoming, Error> {
        let p = &self.provider;
        let pg = p
            .authority
            .grant(p.local, Action::Invoke, 16_777_216)
            .unwrap();
        let sg = p
            .services
            .grant(p.local, services::authority::Action::Invoke, 16_777_216)
            .unwrap();
        let cg = p
            .services
            .grant(p.local, services::authority::Action::Coordinate, 16_777_216)
            .unwrap();
        let pc = context(&pg);
        let sc = service_context(&sg);
        let cc = service_context(&cg);
        let bg = p
            .board_authority
            .grant(p.local, board::authority::Action::Select, 16_777_216)
            .unwrap();
        let bc = board::contract::Context {
            grant: &bg,
            deadline: pc.deadline,
            cancelled: false,
        };
        let current = services::quote::BoardCurrent {
            owner: &p.board,
            context: &bc,
        };
        self.issuer.accept(
            &pc,
            Admission {
                quote_context: &sc,
                coordinate_context: &cc,
                current: &current,
                runtime: p.runtime.as_ref(),
            },
            issued,
            observed,
            message,
        )
    }
    // Run complete accepted input through actual terminal gates and commit the real provider participants once.
    pub fn commit_provider(&self, incoming: Incoming, input: &services::value::Value) {
        let p = &self.provider;
        let a = incoming.association();
        let pg = p
            .authority
            .grant(p.local, Action::Invoke, 16_777_216)
            .unwrap();
        let pc = context(&pg);
        let cg = p
            .services
            .grant(p.local, services::authority::Action::Coordinate, 16_777_216)
            .unwrap();
        let cc = service_context(&cg);
        let mut transfer = crate::modules::peer_protocol::input_transfer::InputTransfer::new(
            &pc,
            p.authority.clone(),
            incoming,
        )
        .unwrap();
        let encoded =
            crate::modules::peer_protocol::value::encode(input, Limits::default(), false).unwrap();
        for (id, body) in encoded.bodies.iter().enumerate() {
            let mut offset = 0;
            for chunk in body.bytes().chunks(65_536) {
                transfer
                    .body(
                        &pc,
                        a,
                        &crate::modules::peer_protocol::record::Record::Body {
                            id: id as u32,
                            offset,
                            data: chunk.to_vec(),
                        },
                    )
                    .unwrap();
                offset += chunk.len() as u64;
            }
        }
        transfer
            .marker(
                &pc,
                a,
                &Message::parse(b"{\"type\":\"input.end\"}", 65536).unwrap(),
            )
            .unwrap();
        transfer
            .end(&pc, a)
            .unwrap()
            .dispatch(&cc, p.runtime.as_ref())
            .unwrap();
        let key = crate::modules::peer_protocol::acceptance::peer(a).key;
        p.runtime.stage(&cc, key).unwrap();
        p.runtime.prepare(&cc, key).unwrap();
        p.runtime
            .decide(&cc, key, services::runtime::Decision::Commit)
            .unwrap();
        p.runtime.commit(&cc, key).unwrap();
    }
    // Read the independent requester's actual owned-credit availability, not the provider's issued map.
    pub fn owned_available(&self) -> u64 {
        let r = &self.requester;
        let read = r
            .ledger_authority
            .grant(
                r.local,
                crate::modules::ledger::authority::Action::Read,
                16_777_216,
            )
            .unwrap();
        r.ledger
            .available(
                &super::world::ledger_context(&read),
                self.provider.local,
                true,
            )
            .unwrap()
    }
}
