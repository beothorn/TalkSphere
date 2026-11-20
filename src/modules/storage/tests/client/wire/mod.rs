//! Public Board/Peer Protocol/Services/Ledger requester integration with controlled remote quotes/results, not a running Storage provider.
mod checks;
mod payment;
mod quote;
#[cfg(target_os = "linux")]
mod recovery;
mod result;
mod wanted;
use super::*;
use crate::modules::{
    board::{
        self,
        cache::{Cache, Provenance},
        selection::Selection,
    },
    ledger,
    peer_protocol::{
        self as wire,
        acceptance::{Admission, Outgoing},
        contract::{Association, Limits as WireLimits},
        message::Message,
        requester::{Client as WireClient, Spending},
    },
    services::{
        self,
        handler::Invocation,
        registry::Registry,
        runtime::{Coordinator, Payment},
    },
};
use serde_json::{Value as Json, json};
use std::time::{Duration, Instant};
const CREDIT: u64 = 100_000;
struct Endpoint<
    L: ledger::persistence::Persistence = ledger::persistence::Memory,
    S: services::persistence::Persistence = services::persistence::Memory,
> {
    client: WireClient,
    payment: Payment,
    family: Arc<Client>,
    registry: Arc<Registry>,
    selection: Selection,
    association: Association,
    authority: Arc<wire::authority::Authority>,
    services: Arc<services::authority::Authority>,
    ledger: Arc<ledger::state::Ledger<L>>,
    ledger_authority: Arc<ledger::authority::Authority>,
    runtime: Coordinator<S>,
}
struct Captured(Selection);
impl services::quote::Current for Captured {
    // A controlled local quote-currentness port substitutes only the unavailable remote publication owner in these requester tests.
    fn check(&self, selection: &Selection) -> Result<(), Error> {
        if selection.roles() != self.0.roles()
            || selection.offering().source != self.0.offering().source
            || !selection.contract_matches(&self.0)
        {
            return Err(Error::Conflict);
        }
        Ok(())
    }
}
// Fresh local call lifetimes do not replace the deadlines already retained by requester proposal/consent objects.
fn pc(grant: &wire::authority::Grant) -> wire::contract::Context<'_> {
    wire::contract::Context {
        version: 1,
        grant,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    }
}
// Services has a separate action/identity authority domain from Peer Protocol.
fn sc(grant: &services::authority::Grant) -> services::contract::Context<'_> {
    services::contract::Context {
        version: 1,
        grant,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    }
}
// Ledger setup and reads require independently issued local grants; fixture credits are explicit finite administrative setup.
fn lc(grant: &ledger::authority::Grant) -> ledger::contract::Context<'_> {
    ledger::contract::Context {
        version: 1,
        grant,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    }
}
// Real requester-only registration and a directly attributed cached contract supply a Selection without installing any provider.
fn endpoint(client: Client, inv: &Invocation) -> Endpoint {
    endpoint_with_payment(client, inv, inv.provider, Payment::default())
}
// Keep the authenticated roles fixed while selecting an explicit issuer and immutable transfer/fee binding.
fn endpoint_with_payment(
    client: Client,
    inv: &Invocation,
    issuer: IdentityId,
    payment: Payment,
) -> Endpoint {
    endpoint_with_stores(
        client,
        inv,
        issuer,
        payment,
        ledger::persistence::Memory::default(),
        services::persistence::Memory::default(),
        true,
    )
}
// Open each independent owner with its own adapter; recovery must not reinitialize balances or retained operation records.
fn endpoint_with_stores<
    L: ledger::persistence::Persistence + 'static,
    S: services::persistence::Persistence,
>(
    client: Client,
    inv: &Invocation,
    issuer: IdentityId,
    payment: Payment,
    ledger_store: L,
    service_store: S,
    initialize: bool,
) -> Endpoint<L, S> {
    let family = Arc::new(client);
    let services = Arc::new(services::authority::Authority::new());
    let catalogue = Arc::new(Catalogue::new(BoardLimits::default()).unwrap());
    let registry = Arc::new(
        Registry::new(
            inv.requester,
            services.clone(),
            Default::default(),
            catalogue,
        )
        .unwrap(),
    );
    let register = services
        .grant(
            inv.requester,
            services::authority::Action::Register,
            16_777_216,
        )
        .unwrap();
    registry
        .register(&sc(&register), family.clone(), None)
        .unwrap();
    let ba = Arc::new(board::authority::Authority::new());
    let mut cache = Cache::new(ba.clone(), BoardLimits::default()).unwrap();
    let grant = ba
        .grant(inv.provider, board::authority::Action::Cache, 16_777_216)
        .unwrap();
    let bc = board::contract::Context {
        grant: &grant,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    };
    // Select the test currency explicitly; the pinned fixture has an unrelated illustrative issuer.
    let mut offering = inv.offering.source.clone();
    offering["creditIssuer"] = crate::modules::identity::text::format(issuer).into();
    let raw = json!({"version":1,"providedServices":[offering],"wantedServices":[]});
    let remote = cache
        .store(
            &bc,
            Provenance::Direct(inv.provider),
            &serde_json::to_vec(&raw).unwrap(),
            100,
        )
        .unwrap();
    let remote = cache
        .attach(
            &bc,
            &remote,
            &inv.offering.id,
            inv.definition.document.bytes(),
            100,
        )
        .unwrap();
    let select = ba
        .grant(inv.provider, board::authority::Action::Select, 16_777_216)
        .unwrap();
    let selection = cache
        .select(
            &board::contract::Context {
                grant: &select,
                ..bc
            },
            &remote,
            &board::selection::Request {
                id: &inv.offering.id,
                service: "cr2se.storage",
                version: 1,
                description: &inv.offering.description,
                other: inv.requester,
                authenticated: true,
            },
            registry.as_ref(),
            100,
        )
        .unwrap();
    let authority = Arc::new(wire::authority::Authority::new());
    let client = WireClient::new(
        inv.requester,
        authority.clone(),
        services.clone(),
        registry.clone(),
        WireLimits::default(),
        Default::default(),
    )
    .unwrap();
    let ledger_authority = Arc::new(ledger::authority::Authority::new());
    let admin = ledger_authority
        .grant(
            inv.requester,
            if initialize {
                ledger::authority::Action::Administer
            } else {
                ledger::authority::Action::Read
            },
            16_777_216,
        )
        .unwrap();
    let ledger = Arc::new(if initialize {
        let ledger = ledger::state::Ledger::initialize(
            inv.requester,
            ledger_authority.clone(),
            ledger_store,
            Default::default(),
            &lc(&admin),
            CREDIT,
            CREDIT,
        )
        .unwrap();
        if issuer != inv.requester {
            ledger.set_owned(&lc(&admin), issuer, CREDIT).unwrap();
        }
        ledger
    } else {
        ledger::state::Ledger::open(
            inv.requester,
            ledger_authority.clone(),
            ledger_store,
            Default::default(),
            &lc(&admin),
        )
        .unwrap()
    });
    let coordinate = ledger_authority
        .grant(
            inv.requester,
            ledger::authority::Action::Coordinate,
            16_777_216,
        )
        .unwrap();
    let accounting = Arc::new(services::accounting::Ledger::new(
        ledger.clone(),
        coordinate,
        Default::default(),
    ));
    let recover = services
        .grant(
            inv.requester,
            services::authority::Action::Recover,
            16_777_216,
        )
        .unwrap();
    let runtime = Coordinator::open(
        services::runtime::Config {
            local: inv.requester,
            authority: services.clone(),
            limits: Default::default(),
        },
        &sc(if initialize { &register } else { &recover }),
        service_store,
        accounting,
        registry.clone(),
        initialize,
    )
    .unwrap();
    let association = Association {
        local: inv.requester,
        peer: inv.provider,
        operation: wire::contract::OperationKey {
            requester: inv.key.requester,
            connection: inv.key.connection,
            stream: inv.key.stream,
        },
    };
    Endpoint {
        client,
        payment,
        family,
        registry,
        selection,
        association,
        authority,
        services,
        ledger,
        ledger_authority,
        runtime,
    }
}
impl<L: ledger::persistence::Persistence, S: services::persistence::Persistence> Endpoint<L, S> {
    // The literal incoming remote quote is independent of the Storage provider encoder and uses exact specified factor widths.
    fn quote(&self, price: u64, factors: Json) -> Message {
        let mut quote = json!({"type":"quote","quoteId":wire::base64::encode(&[6;32]),"offering":self.selection.offering().source,"creditIssuer":crate::modules::identity::text::format(self.selection.roles().2),"priceMode":"fixed","maximumPrice":price.to_string(),"pricingInputs":factors});
        if let (Some(id), Some(fee)) = (self.payment.transfer, self.payment.fee) {
            quote["transferId"] = json!(wire::base64::encode(&id));
            quote["transferFee"] = json!(fee.to_string());
        }
        Message::parse(&serde_json::to_vec(&quote).unwrap(), 65536).unwrap()
    }
    // Only local input ownership, role-scoped grants and explicit spending create a proposal; no accept or body can be sent yet.
    fn request(&self, input: Value, maximum: u64) -> wire::requester::Request {
        let grant = self
            .authority
            .grant(
                self.association.local,
                wire::authority::Action::Request,
                16_777_216,
            )
            .unwrap();
        let request = self
            .services
            .grant(
                self.association.local,
                services::authority::Action::Request,
                16_777_216,
            )
            .unwrap();
        self.client
            .request(
                &pc(&grant),
                &sc(&request),
                self.association,
                self.selection.clone(),
                input,
                Spending {
                    maximum,
                    payment: self.payment,
                },
            )
            .unwrap()
    }
    // An authenticated original quote becomes only a candidate; body/resource/credit ownership remains uncommitted.
    fn candidate(
        &self,
        request: wire::requester::Request,
        quote: &Message,
    ) -> Result<wire::requester::Candidate, wire::Error> {
        let invoke = self
            .authority
            .grant(
                self.association.local,
                wire::authority::Action::Invoke,
                16_777_216,
            )
            .unwrap();
        let request_grant = self
            .services
            .grant(
                self.association.local,
                services::authority::Action::Request,
                16_777_216,
            )
            .unwrap();
        self.client.validate(
            &pc(&invoke),
            &sc(&request_grant),
            request,
            self.association,
            quote,
        )
    }
    // Reserve only the explicitly approved exact quote and original input through real requester Services and Ledger owners.
    fn reserve(&self, input: Value, price: u64, factors: Json) -> Outgoing {
        let request = self.request(input, CREDIT);
        let candidate = self
            .candidate(request, &self.quote(price, factors))
            .unwrap();
        let current = Captured(candidate.selection().clone());
        self.reserve_candidate(candidate, &current)
    }
    // Complete consent/admission against the caller's current publication authority rather than deriving authority from quote bytes.
    fn reserve_candidate(
        &self,
        candidate: wire::requester::Candidate,
        current: &dyn services::quote::Current,
    ) -> Outgoing {
        let hash = candidate.hash();
        let grant = self
            .authority
            .grant(
                self.association.local,
                wire::authority::Action::Request,
                16_777_216,
            )
            .unwrap();
        let request = self
            .services
            .grant(
                self.association.local,
                services::authority::Action::Request,
                16_777_216,
            )
            .unwrap();
        let coordinate = self
            .services
            .grant(
                self.association.local,
                services::authority::Action::Coordinate,
                16_777_216,
            )
            .unwrap();
        let consent = self
            .client
            .consent(&pc(&grant), candidate, hash, CREDIT)
            .unwrap();
        self.client
            .reserve(
                &pc(&grant),
                Admission {
                    quote_context: &sc(&request),
                    coordinate_context: &sc(&coordinate),
                    current,
                    runtime: &self.runtime,
                },
                consent,
            )
            .unwrap()
    }
    // Read actual remaining owned credit after unresolved holds, rather than inferring payment from an application outcome.
    fn available(&self) -> u64 {
        let read = self
            .ledger_authority
            .grant(
                self.association.local,
                ledger::authority::Action::Read,
                16_777_216,
            )
            .unwrap();
        let issuer = self.selection.roles().2;
        if issuer == self.association.local {
            self.ledger.capacities(&lc(&read)).unwrap().0
        } else {
            self.ledger.available(&lc(&read), issuer, true).unwrap()
        }
    }
    // Inspect a requester coordinator only under its original local Observe authority.
    fn phase(&self) -> Option<services::runtime::Phase> {
        let read = self
            .services
            .grant(
                self.association.local,
                services::authority::Action::Observe,
                16_777_216,
            )
            .unwrap();
        self.runtime.status(&sc(&read), self.key()).unwrap()
    }
    // Convert original fields explicitly across public module contract types; no journal or wire payload supplies the key.
    fn key(&self) -> OperationKey {
        OperationKey {
            requester: self.association.operation.requester,
            connection: self.association.operation.connection,
            stream: self.association.operation.stream,
        }
    }
}
