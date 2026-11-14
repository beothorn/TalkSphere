//! Independently owned real check stacks use finite scripted entropy/clocks and explicit local custom resource rules.
use super::{
    invocation::Pair,
    world::{World, context, service_context},
};
use crate::modules::{
    board,
    peer_protocol::{
        self as peer, Error,
        authority::Action,
        checks::{self, clock::Clock},
        contract::{Association, Limits},
        message::Message,
        proposal::Authorizer,
        quote::Entropy,
        requester::Spending,
    },
    services,
};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, Ordering},
    },
    time::{Duration, Instant},
};
struct Random(AtomicU8);
impl Entropy for Random {
    // Fresh deterministic IDs are consumed once; exhaustion refuses rather than reusing one.
    fn quote_id(&self) -> Result<[u8; 32], Error> {
        self.0
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_add(1))
            .map(|n| [n; 32])
            .map_err(|_| Error::Unavailable)
    }
}
pub(in crate::modules::peer_protocol) struct ScriptClock(
    Mutex<VecDeque<(Instant, services::checks::Stamp)>>,
);
impl ScriptClock {
    // Finite trusted local observations let tests advance response timing without sleeping or peer timestamps.
    pub fn new(milliseconds: &[u64]) -> Self {
        let base = Instant::now();
        Self(Mutex::new(
            milliseconds
                .iter()
                .map(|m| {
                    (
                        base + Duration::from_millis(*m),
                        services::checks::Stamp {
                            epoch: [4; 32],
                            milliseconds: *m,
                        },
                    )
                })
                .collect(),
        ))
    }
}
impl Clock for ScriptClock {
    // Unexpected timing observations fail closed and expose integration mistakes in callers.
    fn mark(&self) -> Result<(Instant, services::checks::Stamp), Error> {
        self.0.lock().unwrap().pop_front().ok_or(Error::Unavailable)
    }
}
pub(in crate::modules::peer_protocol) struct Checks {
    pub pair: Pair,
    pub provider: checks::Owner,
    pub requester: checks::Owner,
}
// The peer owns wire/grants only; actual financial state and resource rules stay with their explicit public module owners.
fn owner(w: &World) -> checks::Owner {
    checks::Owner::new(checks::Config {
        local: w.local,
        authority: w.authority.clone(),
        services: w.services.clone(),
        registry: w.registry.clone(),
        limits: Limits::default(),
        gate: w.gate.clone(),
        entropy: Arc::new(Random(AtomicU8::new(1))),
    })
    .unwrap()
}
impl Checks {
    // Paid or included custom resources are installed before real Board publication on both independent owners.
    pub fn new(included: bool) -> Self {
        let pair = Pair::checks(included);
        let provider = owner(&pair.provider);
        let requester = owner(&pair.requester);
        Self {
            pair,
            provider,
            requester,
        }
    }
    // Send actual local challenge proposal bytes to provider authorization before constructing an exact check quote.
    pub fn quote(
        &self,
        stream: u32,
        challenge: services::value::Value,
        maximum: u64,
    ) -> Result<(checks::Issued, checks::Request), Error> {
        let p = &self.pair.provider;
        let r = &self.pair.requester;
        let (pa, selected, _) = p.proposal(
            stream,
            &services::value::Value::String("unused ordinary input".into()),
        );
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
        let request = self.requester.request(
            &context(&rg),
            &service_context(&rsg),
            ra,
            selected.clone(),
            challenge,
            Spending {
                maximum,
                payment: Default::default(),
            },
        )?;
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
        let proposal = authorizer.authorize(
            &pc,
            &sc,
            pa,
            selected,
            &Message::parse(request.bytes(), 65536)?,
        )?;
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
        Ok((
            self.provider
                .issue(&pc, &sc, proposal, &current, pc.deadline)?,
            request,
        ))
    }
    // Requester exact consent creates a real hold and once-only outgoing marker before accept bytes are exposed.
    pub fn reserve(
        &self,
        issued: &checks::Issued,
        request: checks::Request,
    ) -> Result<checks::Outgoing, Error> {
        self.reserve_message(
            issued.association(),
            request,
            &Message::parse(issued.bytes(), 65536)?,
        )
    }
    // Bind actual composed endpoint quote bytes without borrowing its private provider state.
    pub fn reserve_message(
        &self,
        association: Association,
        request: checks::Request,
        message: &Message,
    ) -> Result<checks::Outgoing, Error> {
        let p = &self.pair.provider;
        let r = &self.pair.requester;
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
        let a = Association {
            local: r.local,
            peer: p.local,
            ..association
        };
        let candidate = self
            .requester
            .validate(&context(&ig), &sc, request, a, message)?;
        let hash = candidate.hash();
        let consent = self.requester.consent(&c, candidate, hash, 100)?;
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
        self.requester.reserve(
            &c,
            peer::acceptance::Admission {
                quote_context: &sc,
                coordinate_context: &cc,
                current: &current,
                runtime: r.runtime.as_ref(),
            },
            consent,
        )
    }
    // Provider independent hold follows receipt of original ID/hash acceptance; no evidence is produced here.
    pub fn accept(&self, issued: checks::Issued, accept: &[u8]) -> Result<checks::Incoming, Error> {
        let p = &self.pair.provider;
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
        let c = context(&pg);
        let sc = service_context(&sg);
        let cc = service_context(&cg);
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
        let a = issued.association();
        self.provider.accept(
            &c,
            peer::acceptance::Admission {
                quote_context: &sc,
                coordinate_context: &cc,
                current: &current,
                runtime: p.runtime.as_ref(),
            },
            issued,
            a,
            &Message::parse(accept, 65536)?,
        )
    }
    // Provider complete evidence is committed through the real Ledger before a sender can borrow it.
    pub fn commit(
        &self,
        dispatched: checks::input::Dispatched,
        association: Association,
    ) -> Result<checks::evidence::Sender, Error> {
        let p = &self.pair.provider;
        let cg = p
            .services
            .grant(p.local, services::authority::Action::Coordinate, 16_777_216)
            .unwrap();
        let cc = service_context(&cg);
        let key = peer::acceptance::peer(association).key;
        p.runtime.prepare(&cc, key)?;
        p.runtime
            .decide(&cc, key, services::runtime::Decision::Commit)?;
        p.runtime.commit(&cc, key)?;
        let og = p
            .services
            .grant(p.local, services::authority::Action::Observe, 16_777_216)
            .unwrap();
        let oc = service_context(&og);
        let pg = p
            .authority
            .grant(p.local, Action::Invoke, 16_777_216)
            .unwrap();
        checks::evidence::Sender::new(
            &context(&pg),
            p.authority.as_ref(),
            &oc,
            p.runtime.as_ref(),
            dispatched,
            Limits::default(),
        )
    }
}
