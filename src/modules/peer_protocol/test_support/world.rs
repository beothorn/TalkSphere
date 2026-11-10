//! Actual Board/Services/Ledger fixture over public contracts, with explicit finite administrative setup.
use super::family::Echo;
use crate::modules::{
    board,
    identity::{api::IdentityId, text},
    ledger,
    peer_protocol::{
        admission::{Gate, LedgerGate},
        authority::{Authority, Grant},
        contract::{Association, Context, Limits, OperationKey},
        message::Message,
        value,
    },
    services::{self, handler::Requester},
};
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};
pub(in crate::modules::peer_protocol) struct World {
    pub local: IdentityId,
    pub requester: IdentityId,
    pub authority: Arc<Authority>,
    pub services: Arc<services::authority::Authority>,
    pub registry: Arc<services::registry::Registry>,
    pub board: board::state::Provider,
    pub board_authority: Arc<board::authority::Authority>,
    pub ledger: Arc<ledger::state::Ledger<ledger::persistence::Memory>>,
    pub ledger_authority: Arc<ledger::authority::Authority>,
    pub family: Arc<Echo>,
    pub gate: Arc<dyn Gate>,
    pub runtime: Arc<services::runtime::Coordinator<services::persistence::Memory>>,
}
// Every boundary uses a separately issued finite grant and current monotonic call lifetime.
pub(in crate::modules::peer_protocol) fn context(grant: &Grant) -> Context<'_> {
    Context {
        version: 1,
        grant,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    }
}
// Services owns a distinct authority domain; the wire caller never receives its issuer.
pub(in crate::modules::peer_protocol) fn service_context(
    grant: &services::authority::Grant,
) -> services::contract::Context<'_> {
    services::contract::Context {
        version: 1,
        grant,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    }
}
// Ledger read/admin/coordinator controls cannot be reconstructed from application JSON.
pub(in crate::modules::peer_protocol) fn ledger_context(
    grant: &ledger::authority::Grant,
) -> ledger::contract::Context<'_> {
    ledger::contract::Context {
        version: 1,
        grant,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    }
}
impl World {
    // Configure a provider-issued one-hundred-credit fixture and an explicitly installed custom handler.
    pub(in crate::modules::peer_protocol) fn new(bytes: bool) -> Self {
        let local = IdentityId([2; 32]);
        let requester = IdentityId([1; 32]);
        let authority = Arc::new(Authority::new());
        let services = Arc::new(services::authority::Authority::new());
        let family = Arc::new(Echo::new(bytes));
        let registry = Arc::new(
            services::registry::Registry::new(
                local,
                services.clone(),
                Default::default(),
                Arc::new(services::registry::NoStandards),
            )
            .unwrap(),
        );
        let register = services
            .grant(local, services::authority::Action::Register, 16_777_216)
            .unwrap();
        registry
            .register(
                &service_context(&register),
                family.clone(),
                Some(family.clone()),
            )
            .unwrap();
        let board_authority = Arc::new(board::authority::Authority::new());
        let owner = board::state::Provider::new(
            local,
            board_authority.clone(),
            Default::default(),
            registry.clone(),
        )
        .unwrap();
        let offering = family.offering(local);
        let raw=serde_json::json!({"version":1,"providedServices":[offering.source],"wantedServices":[]}).to_string();
        let prepared = board::prepared::Prepared::new(
            local,
            raw.as_bytes(),
            &BTreeMap::from([(
                "fixture".into(),
                family.definition().document.bytes().to_vec(),
            )]),
            Default::default(),
            registry.as_ref(),
        )
        .unwrap();
        let publish = board_authority
            .grant(local, board::authority::Action::Publish, 16_777_216)
            .unwrap();
        owner
            .publish(
                &board::contract::Context {
                    grant: &publish,
                    deadline: Instant::now() + Duration::from_secs(60),
                    cancelled: false,
                },
                prepared,
            )
            .unwrap();
        let ledger_authority = Arc::new(ledger::authority::Authority::new());
        let admin = ledger_authority
            .grant(local, ledger::authority::Action::Administer, 16_777_216)
            .unwrap();
        let ledger = Arc::new(
            ledger::state::Ledger::initialize(
                local,
                ledger_authority.clone(),
                ledger::persistence::Memory::default(),
                Default::default(),
                &ledger_context(&admin),
                1000,
                100,
            )
            .unwrap(),
        );
        ledger
            .grant(&ledger_context(&admin), requester, 100)
            .unwrap();
        ledger
            .set_trust(
                &ledger_context(&admin),
                requester,
                ledger::accounting::Trust::new(1).unwrap(),
            )
            .unwrap();
        let read = ledger_authority
            .grant(local, ledger::authority::Action::Read, 16_777_216)
            .unwrap();
        let gate =
            Arc::new(LedgerGate::new(local, ledger.clone(), read, Default::default()).unwrap());
        let coordinate = ledger_authority
            .grant(local, ledger::authority::Action::Coordinate, 16_777_216)
            .unwrap();
        let accounting = Arc::new(services::accounting::Ledger::new(
            ledger.clone(),
            coordinate,
            Default::default(),
        ));
        let runtime = Arc::new(
            services::runtime::Coordinator::open(
                services::runtime::Config {
                    local,
                    authority: services.clone(),
                    limits: Default::default(),
                },
                &service_context(&register),
                services::persistence::Memory::default(),
                accounting,
                registry.clone(),
                true,
            )
            .unwrap(),
        );
        Self {
            local,
            requester,
            authority,
            services,
            registry,
            board: owner,
            board_authority,
            ledger,
            ledger_authority,
            family,
            gate,
            runtime,
        }
    }
    // Select the current actual publication and build immutable proposal bytes on a fresh synthetic authenticated association.
    pub(in crate::modules::peer_protocol) fn proposal(
        &self,
        stream: u32,
        input: &services::value::Value,
    ) -> (Association, board::selection::Selection, Message) {
        let read = self
            .board_authority
            .grant(self.local, board::authority::Action::Read, 16_777_216)
            .unwrap();
        let select = self
            .board_authority
            .grant(self.local, board::authority::Action::Select, 16_777_216)
            .unwrap();
        let c = board::contract::Context {
            grant: &read,
            deadline: Instant::now() + Duration::from_secs(60),
            cancelled: false,
        };
        let snapshot = self.board.read(&c).unwrap();
        let o = self.family.offering(self.local);
        let selected = self
            .board
            .select(
                &board::contract::Context {
                    grant: &select,
                    ..c
                },
                &snapshot,
                &board::selection::Request {
                    id: &o.id,
                    service: &o.service,
                    version: 1,
                    description: &o.description,
                    other: self.requester,
                    authenticated: true,
                },
            )
            .unwrap();
        let association = Association {
            local: self.local,
            peer: self.requester,
            operation: OperationKey {
                requester: self.requester,
                connection: [8; 32],
                stream,
            },
        };
        let metadata = value::encode(input, Limits::default(), false)
            .unwrap()
            .metadata;
        let raw=serde_json::json!({"type":"propose","offeringId":"fixture","publisherId":text::format(self.local),"service":o.service,"serviceVersion":1,"mode":"invoke","input":metadata,"creditIssuer":text::format(self.local),"maximumPrice":"7"}).to_string();
        (
            association,
            selected,
            Message::parse(raw.as_bytes(), 65536).unwrap(),
        )
    }
    // Read actual finite issued capacity to prove preflight did not allocate financial holds.
    pub(in crate::modules::peer_protocol) fn available(&self) -> u64 {
        let g = self
            .ledger_authority
            .grant(self.local, ledger::authority::Action::Read, 16_777_216)
            .unwrap();
        self.ledger
            .available(&ledger_context(&g), self.requester, false)
            .unwrap()
    }
}
