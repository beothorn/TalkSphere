//! Control command handler: local accounting commands, inspection and the paid Storage requests.
//!
//! Callers: the control socket workers. Administrative commands change only this node's own
//! Ledger. Peer commands connect through the node loop, spend only under an explicit caller
//! bound, validate every result with the Storage requester contracts and update the lease
//! catalog only afterwards. Nothing here retries accepted work: a lost reply is reported as an
//! unknown outcome and recorded in the operations journal.
use super::{
    journal::{self, Journal, Record, Status},
    protocol::{Code, ControlError, Request, hex},
    socket::Execute,
};
use crate::{
    composition::{
        node::{
            catalog::Entry,
            link::Link,
            owners::{BUDGET, Owners},
        },
        server::pump::Signal,
    },
    modules::{
        identity::{api::IdentityId, text},
        ledger,
        services::{self, Error as ServiceError},
    },
};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

mod peer;

/// Most grant replies remembered for resent request identifiers.
const REMEMBERED_GRANTS: usize = 256;
/// Authority lifetime budget for locally issued grants.
const GRANT_BYTES: usize = 16_777_216;

/// A grant request already carried out, kept so a resend returns the same reply without a second grant.
struct Granted {
    id: u64,
    parameters: (IdentityId, u64),
    reply: Value,
}

/// Everything a command needs; shared by every control worker.
pub struct Handler {
    pub(super) owners: Arc<Owners>,
    pub(super) link: Link,
    pub(super) signal: Signal,
    pub(super) journal: Journal,
    listen: SocketAddr,
    timeout: Duration,
    granted: Mutex<VecDeque<Granted>>,
}

impl Handler {
    /// Build the handler for a ready node.
    /// # Errors
    /// The operations journal is unreadable or malformed.
    pub fn new(
        owners: Arc<Owners>,
        link: Link,
        signal: Signal,
        listen: SocketAddr,
        journal_path: &std::path::Path,
    ) -> Result<Self, String> {
        tracing::trace!("Building the control command handler");
        let timeout = Duration::from_secs(owners.config.control.request_seconds);
        Ok(Self {
            journal: Journal::open(journal_path)?,
            owners,
            link,
            signal,
            listen,
            timeout,
            granted: Mutex::new(VecDeque::new()),
        })
    }

    // Parse a peer identity parameter.
    fn identity(request: &Request, name: &str) -> Result<IdentityId, ControlError> {
        tracing::trace!("Reading an identity control parameter");
        text::parse(request.text(name)?).map_err(|_| {
            ControlError::new(
                Code::Invalid,
                format!("{name} must be a canonical CR2SE ID"),
            )
        })
    }

    // Build a Ledger context under a fresh scoped grant.
    fn with_ledger<T>(
        &self,
        action: ledger::authority::Action,
        run: impl FnOnce(&ledger::contract::Context<'_>) -> Result<T, ledger::Error>,
    ) -> Result<T, ControlError> {
        tracing::trace!("Running a Ledger operation under a scoped grant");
        let grant = self
            .owners
            .ledger_authority
            .grant(self.owners.local, action, GRANT_BYTES)
            .map_err(ledger_error)?;
        run(&ledger::contract::Context {
            version: 1,
            grant: &grant,
            deadline: Instant::now() + self.timeout,
            cancelled: false,
        })
        .map_err(ledger_error)
    }

    // Issue finite credits once per request identifier; a resend with the same parameters repeats the reply.
    fn grant(&self, request: &Request) -> Result<Value, ControlError> {
        tracing::trace!("Granting finite credits");
        let (peer, amount) = (Self::identity(request, "peer")?, request.u64("amount")?);
        let mut done = self
            .granted
            .lock()
            .map_err(|_| unavailable("grant history"))?;
        if let Some(previous) = done.iter().find(|g| g.id == request.id) {
            return if previous.parameters == (peer, amount) {
                Ok(previous.reply.clone())
            } else {
                Err(ControlError::new(
                    Code::Invalid,
                    "request id was already used with different parameters",
                ))
            };
        }
        self.with_ledger(ledger::authority::Action::Administer, |c| {
            self.owners.account.grant(c, peer, amount)
        })?;
        let reply = json!({"granted": amount, "peer": text::format(peer)});
        done.push_back(Granted {
            id: request.id,
            parameters: (peer, amount),
            reply: reply.clone(),
        });
        while done.len() > REMEMBERED_GRANTS {
            done.pop_front();
        }
        Ok(reply)
    }

    // Record the credits this node believes it holds from a peer issuer (absolute, so safely repeatable).
    fn own(&self, request: &Request) -> Result<Value, ControlError> {
        tracing::trace!("Recording owned credits");
        let (peer, amount) = (Self::identity(request, "peer")?, request.u64("amount")?);
        self.with_ledger(ledger::authority::Action::Administer, |c| {
            self.owners.account.set_owned(c, peer, amount)
        })?;
        Ok(json!({"owned": amount, "peer": text::format(peer)}))
    }

    // Set exact local trust in millionths (absolute, so safely repeatable).
    fn trust(&self, request: &Request) -> Result<Value, ControlError> {
        tracing::trace!("Setting local trust");
        let peer = Self::identity(request, "peer")?;
        let millionths = u32::try_from(request.u64("millionths")?)
            .map_err(|_| ControlError::new(Code::Invalid, "millionths must be at most 1000000"))?;
        let trust = ledger::accounting::Trust::new(millionths).map_err(ledger_error)?;
        self.with_ledger(ledger::authority::Action::Administer, |c| {
            self.owners.account.set_trust(c, peer, trust)
        })?;
        Ok(json!({"trust": millionths, "peer": text::format(peer)}))
    }

    // Issuer-separated local view for one peer.
    fn balance_of(&self, peer: IdentityId) -> Result<Value, ControlError> {
        tracing::trace!("Reading a peer balance view");
        let view = self.with_ledger(ledger::authority::Action::Read, |c| {
            self.owners.account.view(c, peer)
        })?;
        Ok(json!({
            "peer": text::format(peer), "owned": view.owned, "issued": view.issued,
            "trust": view.trust.millionths(),
        }))
    }

    // Local balance command.
    fn balance(&self, request: &Request) -> Result<Value, ControlError> {
        tracing::trace!("Reporting a balance");
        self.balance_of(Self::identity(request, "peer")?)
    }

    // Cataloged leases, oldest provider first.
    fn leases(&self) -> Value {
        tracing::trace!("Listing cataloged leases");
        Value::Array(self.owners.catalog.list().iter().map(lease_json).collect())
    }

    // Node overview: identity, address, role, leases, unknown outcomes and balances at known providers.
    fn status(&self) -> Result<Value, ControlError> {
        tracing::trace!("Reporting node status");
        let entries = self.owners.catalog.list();
        let mut providers: Vec<IdentityId> = entries.iter().map(|e| e.lease.provider).collect();
        providers.dedup();
        let balances = providers
            .into_iter()
            .map(|p| self.balance_of(p))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(json!({
            "identity": text::format(self.owners.local), "address": self.listen.to_string(),
            "provider": self.owners.provider, "leases": entries.len(),
            "unknownOutcomes": self.journal.unknown().len(), "balances": balances,
            "stopping": self.signal.stopping(),
        }))
    }

    // Recent outcomes; `unknownOnly` narrows to operations needing operator attention.
    fn outcomes(&self, request: &Request) -> Result<Value, ControlError> {
        tracing::trace!("Reporting operation outcomes");
        let only = request
            .params
            .get("unknownOnly")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let records = if only {
            self.journal.unknown()
        } else {
            self.journal.all()
        };
        Ok(Value::Array(records.iter().map(journal::to_json).collect()))
    }

    // Record how a peer operation ended, derived from its result.
    pub(super) fn note(
        &self,
        request: &Request,
        lease: Option<&[u8; 32]>,
        result: &Result<Value, ControlError>,
    ) {
        tracing::trace!("Noting an operation outcome");
        let status = match result {
            Ok(_) => Status::Ok,
            Err(e) if e.code == Code::UnknownOutcome => Status::Unknown,
            Err(_) => Status::Failed,
        };
        self.journal.record(Record {
            id: request.id,
            op: request.op.clone(),
            lease: lease.map(|l| hex(l)),
            status,
            at: now(),
        });
    }

    // Remaining time for one request, from its start.
    pub(super) fn remaining(&self, start: Instant) -> Result<Duration, ControlError> {
        tracing::trace!("Computing remaining request time");
        self.timeout
            .checked_sub(start.elapsed())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| ControlError::new(Code::Unavailable, "the request deadline passed"))
    }
}

impl Execute for Handler {
    // Dispatch one request; unknown operations are refused explicitly.
    fn execute(&self, request: &Request) -> Result<Value, ControlError> {
        tracing::trace!("Executing a control request");
        match request.op.as_str() {
            "status" => self.status(),
            "grant" => self.grant(request),
            "own" => self.own(request),
            "trust" => self.trust(request),
            "balance" => self.balance(request),
            "leases" => Ok(self.leases()),
            "outcomes" => self.outcomes(request),
            "fetch" | "store" | "retrieve" | "renew" | "check" | "remove" => {
                self.peer_operation(request)
            }
            _ => Err(ControlError::new(
                Code::Unsupported,
                "unsupported operation",
            )),
        }
    }
}

// JSON form of one cataloged lease.
pub(super) fn lease_json(entry: &Entry) -> Value {
    let l = &entry.lease;
    json!({
        "leaseId": hex(&l.id), "provider": text::format(l.provider), "contact": entry.contact,
        "sizeBytes": l.size_bytes, "contentHash": hex(&l.content_hash), "acceptedAt": l.accepted_at,
        "expiresAt": l.expires_at, "storePrice": l.store_price,
        "state": match l.state {
            crate::modules::storage::contract::State::Receiving => "receiving",
            crate::modules::storage::contract::State::Active => "active",
            crate::modules::storage::contract::State::Expired => "expired",
            crate::modules::storage::contract::State::Removed => "removed",
        },
    })
}

// Map a Ledger failure to a control error without echoing internals.
fn ledger_error(error: ledger::Error) -> ControlError {
    tracing::trace!("Mapping a Ledger failure");
    use ledger::Error as E;
    match error {
        E::Invalid => ControlError::new(Code::Invalid, "the accounting request is invalid"),
        E::Denied => ControlError::new(Code::Denied, "the accounting request was denied"),
        E::Insufficient => ControlError::new(
            Code::Insufficient,
            "insufficient credits or issuance budget",
        ),
        E::Limit => ControlError::new(Code::Limit, "an accounting limit was reached"),
        _ => ControlError::new(Code::Unavailable, "accounting is unavailable"),
    }
}

// Standard error for an unavailable local dependency.
pub(super) fn unavailable(what: &str) -> ControlError {
    tracing::trace!("Building an unavailable error");
    ControlError::new(Code::Unavailable, format!("{what} is unavailable"))
}

// Current UTC seconds, or zero before the epoch.
pub(super) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

// Map a Services failure to a control error.
pub(super) fn service_error(error: ServiceError) -> ControlError {
    tracing::trace!("Mapping a Services failure");
    match error {
        ServiceError::Denied => ControlError::new(Code::Denied, "the operation was not authorized"),
        ServiceError::Insufficient => {
            ControlError::new(Code::Insufficient, "insufficient credits or spending bound")
        }
        ServiceError::Limit => ControlError::new(Code::Limit, "a finite limit was reached"),
        ServiceError::Unsupported => {
            ControlError::new(Code::Unsupported, "the contract is not supported")
        }
        ServiceError::Unknown => ControlError::new(Code::UnknownOutcome, "the outcome is unknown"),
        ServiceError::Invalid | ServiceError::Conflict => {
            ControlError::new(Code::Refused, "the result was not valid")
        }
        _ => ControlError::new(Code::Unavailable, "a local dependency is unavailable"),
    }
}

// Typed value budget shared with the Storage requester contracts.
pub(super) fn budget() -> services::value::Budget {
    BUDGET
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composition::node::{config::NodeConfig, layout::Layout, owners};
    use serde_json::Map;
    const ME: IdentityId = IdentityId([1; 32]);
    const PEER: IdentityId = IdentityId([2; 32]);
    // A handler over freshly created owners; the loop side of the link is absent, so peer work is unavailable.
    fn handler() -> Handler {
        use std::os::unix::fs::DirBuilderExt;
        let mut nonce = [0; 8];
        getrandom::fill(&mut nonce).unwrap();
        let root =
            std::env::temp_dir().join(format!("node-commands-{}", u64::from_le_bytes(nonce)));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        let layout = Layout::new(&root);
        let owners = Arc::new(owners::open(&layout, NodeConfig::default(), ME, true).unwrap());
        let signal = Signal::default();
        let (link, _commands) = Link::new(signal.clone());
        Handler::new(
            owners,
            link,
            signal,
            "127.0.0.1:1".parse().unwrap(),
            &layout.operations(),
        )
        .unwrap()
    }
    fn request(id: u64, op: &str, params: Value) -> Request {
        let Value::Object(params) = params else {
            return Request {
                id,
                op: op.into(),
                params: Map::new(),
            };
        };
        Request {
            id,
            op: op.into(),
            params,
        }
    }
    fn run(handler: &Handler, id: u64, op: &str, params: Value) -> Result<Value, ControlError> {
        handler.execute(&request(id, op, params))
    }
    fn code(result: Result<Value, ControlError>) -> Code {
        result.unwrap_err().code
    }
    // A grant is applied once per request id: a resend returns the same reply without issuing twice,
    // while reusing the id for other parameters is refused.
    #[test]
    fn grants_apply_once_per_request_id() {
        let h = handler();
        let peer = text::format(PEER);
        let first = run(&h, 7, "grant", json!({"peer": peer, "amount": 100})).unwrap();
        assert_eq!(first["granted"], 100);
        assert_eq!(
            run(&h, 7, "grant", json!({"peer": peer, "amount": 100})).unwrap(),
            first
        );
        assert_eq!(
            run(&h, 7, "balance", json!({"peer": peer})).unwrap()["issued"],
            100
        );
        assert_eq!(
            code(run(&h, 7, "grant", json!({"peer": peer, "amount": 5}))),
            Code::Invalid
        );
        run(&h, 8, "grant", json!({"peer": peer, "amount": 50})).unwrap();
        assert_eq!(
            run(&h, 9, "balance", json!({"peer": peer})).unwrap()["issued"],
            150
        );
    }
    // Issuance is finite, and zero, self and malformed grants are refused without changing balances.
    #[test]
    fn grants_are_finite_and_validated() {
        let h = handler();
        let peer = text::format(PEER);
        assert_eq!(
            code(run(
                &h,
                1,
                "grant",
                json!({"peer": peer, "amount": 1_000_001})
            )),
            Code::Insufficient
        );
        assert_eq!(
            code(run(&h, 2, "grant", json!({"peer": peer, "amount": 0}))),
            Code::Invalid
        );
        assert_eq!(
            code(run(
                &h,
                3,
                "grant",
                json!({"peer": text::format(ME), "amount": 1})
            )),
            Code::Invalid
        );
        assert_eq!(
            code(run(
                &h,
                4,
                "grant",
                json!({"peer": "nonsense", "amount": 1})
            )),
            Code::Invalid
        );
        assert_eq!(
            code(run(&h, 5, "grant", json!({"peer": peer}))),
            Code::Invalid
        );
        assert_eq!(
            code(run(&h, 6, "grant", json!({"peer": peer, "amount": -1}))),
            Code::Invalid
        );
        assert_eq!(
            run(&h, 7, "balance", json!({"peer": peer})).unwrap()["issued"],
            0
        );
    }
    // Owned credits and trust are absolute settings that can be repeated, and trust stays within one million.
    #[test]
    fn owned_credits_and_trust_are_absolute() {
        let h = handler();
        let peer = text::format(PEER);
        for _ in 0..2 {
            run(&h, 1, "own", json!({"peer": peer, "amount": 40})).unwrap();
            run(&h, 2, "trust", json!({"peer": peer, "millionths": 500_000})).unwrap();
        }
        let view = run(&h, 3, "balance", json!({"peer": peer})).unwrap();
        assert_eq!(
            (view["owned"].as_u64(), view["trust"].as_u64()),
            (Some(40), Some(500_000))
        );
        assert_eq!(
            code(run(
                &h,
                4,
                "trust",
                json!({"peer": peer, "millionths": 1_000_001})
            )),
            Code::Invalid
        );
        assert_eq!(
            code(run(
                &h,
                5,
                "trust",
                json!({"peer": peer, "millionths": 5_000_000_000u64})
            )),
            Code::Invalid
        );
    }
    // Inspection reports an empty node and rejects unsupported operations and absent optional families.
    #[test]
    fn inspection_and_unsupported_operations() {
        let h = handler();
        let status = run(&h, 1, "status", json!({})).unwrap();
        assert_eq!(status["identity"], text::format(ME));
        assert_eq!(
            (
                status["leases"].as_u64(),
                status["unknownOutcomes"].as_u64()
            ),
            (Some(0), Some(0))
        );
        assert_eq!(status["provider"], true);
        assert!(
            run(&h, 2, "leases", json!({}))
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(
            run(&h, 3, "outcomes", json!({"unknownOnly": true}))
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty()
        );
        for family in [
            "compute", "message", "discover", "files", "feeds", "pages", "",
        ] {
            assert_eq!(
                code(run(&h, 4, family, json!({}))),
                Code::Unsupported,
                "{family}"
            );
        }
    }
    // Peer operations validate parameters first, refuse unknown leases locally and report an
    // unreachable node loop as unavailable; each outcome is journaled and none is unknown.
    #[test]
    fn peer_operations_fail_safely_without_a_loop() {
        let h = handler();
        let contact = format!("{}?address=127.0.0.1&port=9", text::format(PEER));
        let lease = "00".repeat(32);
        assert_eq!(
            code(run(
                &h,
                1,
                "store",
                json!({"contact": contact, "data": "", "retentionSeconds": 5, "maximum": 9})
            )),
            Code::Invalid
        );
        assert_eq!(
            code(run(
                &h,
                2,
                "store",
                json!({"contact": contact, "data": "zz", "retentionSeconds": 5, "maximum": 9})
            )),
            Code::Invalid
        );
        assert_eq!(
            code(run(
                &h,
                3,
                "store",
                json!({"contact": "nonsense", "data": "00", "retentionSeconds": 5, "maximum": 9})
            )),
            Code::Invalid
        );
        assert_eq!(
            code(run(
                &h,
                4,
                "store",
                json!({"contact": text::format(PEER), "data": "00", "retentionSeconds": 5, "maximum": 9})
            )),
            Code::Invalid
        );
        assert_eq!(
            code(run(
                &h,
                5,
                "store",
                json!({"contact": contact, "data": "00", "retentionSeconds": 5, "maximum": 9})
            )),
            Code::Unavailable
        );
        assert_eq!(
            code(run(&h, 6, "fetch", json!({"contact": contact}))),
            Code::Unavailable
        );
        for op in ["retrieve", "renew", "remove", "check"] {
            let params = json!({"lease": lease, "maximum": 5, "seconds": 5, "offset": 0, "expectedByte": "00"});
            assert_eq!(code(run(&h, 7, op, params)), Code::Denied, "{op}");
        }
        assert_eq!(
            code(run(&h, 8, "retrieve", json!({"lease": "00", "maximum": 5}))),
            Code::Invalid
        );
        assert!(h.journal.unknown().is_empty());
        assert!(h.journal.all().len() >= 5);
        let unavailable = ControlError::new(Code::Unavailable, "x");
        assert_eq!(unavailable.code, Code::Unavailable);
    }
    // Service and ledger failures map to explicit codes without exposing internals.
    #[test]
    fn failures_map_to_codes() {
        assert_eq!(service_error(ServiceError::Denied).code, Code::Denied);
        assert_eq!(
            service_error(ServiceError::Insufficient).code,
            Code::Insufficient
        );
        assert_eq!(service_error(ServiceError::Limit).code, Code::Limit);
        assert_eq!(
            service_error(ServiceError::Unsupported).code,
            Code::Unsupported
        );
        assert_eq!(
            service_error(ServiceError::Unknown).code,
            Code::UnknownOutcome
        );
        assert_eq!(service_error(ServiceError::Invalid).code, Code::Refused);
        assert_eq!(
            service_error(ServiceError::Unavailable).code,
            Code::Unavailable
        );
        assert_eq!(unavailable("catalog").message, "catalog is unavailable");
        assert!(budget().bytes > 0);
    }
}
