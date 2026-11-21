//! Native durable owner fixtures with explicit test-only metadata fault injection and trusted lease seeding.
use super::{
    Write,
    contracts::{HASH, renew},
};
use crate::modules::{
    services::{
        Error,
        handler::{Blob, Completion, Invocation},
        runtime::Phase,
        value::{Budget, Value},
    },
    storage::{
        bytes::{
            Store,
            tests::{PathGuard, path},
        },
        contract::{Crypto, Limits, Persistence, Time},
        effect::Effect,
        image::{Limits as ImageLimits, Stage},
        journal::Journal,
        recovery::{Gate, Resume},
        retention::Anchor,
    },
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
pub(crate) const B: Budget = Budget {
    depth: 8,
    nodes: 128,
    bytes: 4096,
};
pub(crate) struct Faults {
    pub store: Arc<Store>,
    pub writes: Mutex<VecDeque<Write>>,
}
impl Persistence for Faults {
    // Read actual protected on-disk metadata; fault scripts affect only replacement acknowledgements.
    fn load(&self, maximum: usize) -> Result<Option<Vec<u8>>, Error> {
        self.store.load(maximum)
    }
    // Inject confirmed failure or durable lost acknowledgement without inventing rollback after a write.
    fn replace(&self, bytes: &[u8]) -> Result<(), Error> {
        let action = self
            .writes
            .lock()
            .map_err(|_| Error::Unavailable)?
            .pop_front()
            .unwrap_or(Write::Commit);
        if matches!(action, Write::Fail) {
            return Err(Error::Unavailable);
        }
        self.store.replace(bytes)?;
        if matches!(action, Write::LoseReply) {
            Err(Error::Unknown)
        } else {
            Ok(())
        }
    }
}
pub(crate) struct CryptoPort;
impl Crypto for CryptoPort {
    // Deterministic test entropy is never used by production or advertised as secure networking.
    fn lease_id(&self) -> Result<[u8; 32], Error> {
        Ok([7; 32])
    }
    // Actual hashing preserves independent token bindings and immutable object verification.
    fn hash(&self, bytes: &[u8]) -> Result<[u8; 32], Error> {
        crate::modules::cryptography::primitives::hash::sha256(bytes, bytes.len())
            .map_err(|_| Error::Limit)
    }
}
pub(crate) struct Fixture {
    pub path: PathGuard,
    pub journal: Arc<Journal>,
    pub bytes: Arc<Store>,
    pub faults: Arc<Faults>,
    pub gate: Gate,
    pub invocation: Invocation,
    pub input: Value,
    pub output: Value,
}
pub(crate) struct Terminal<'a>(pub &'a Journal);
impl Resume for Terminal<'_> {
    // The substitute coordinator is local to the original provider, never chosen by peer payloads.
    fn local_identity(&self) -> crate::modules::identity::api::IdentityId {
        self.0.snapshot().unwrap().provider
    }
    // Expose only terminal original records; this substitute never executes new work or resolves uncertain decisions.
    fn resume(
        &self,
        key: crate::modules::services::contract::OperationKey,
    ) -> Result<Phase, Error> {
        match self.0.status(key)? {
            Some(Stage::Committed(_)) => Ok(Phase::Committed),
            Some(Stage::Aborted(_)) => Ok(Phase::Aborted),
            _ => Err(Error::Unknown),
        }
    }
}
// Finite owner budgets permit competing operations while retaining original results and ID history.
pub(crate) fn limits() -> Limits {
    Limits {
        bytes: 64,
        leases: 8,
        operations: 8,
        result_bytes: 32768,
        per_owner: 8,
    }
}
// Independent logical and encoded bounds constrain every durable owner snapshot and complete value.
pub(crate) fn image_limits() -> ImageLimits {
    ImageLimits {
        bytes: 65536,
        leases: 8,
        operations: 16,
        values: B,
    }
}
// Stable boot observations let tests advance retention independently of wall-clock corrections.
pub(crate) fn time(elapsed: u64) -> Time {
    Time {
        unix_seconds: 1000 + elapsed,
        elapsed_seconds: elapsed,
        boot: [1; 16],
    }
}
// Seed a trusted durable lease fixture directly; this is not paid store creation and selects no acceptance-time workaround.
pub(crate) fn fixture() -> Fixture {
    let path = path();
    let bytes = Arc::new(Store::open(&path.0, true).unwrap());
    let faults = Arc::new(Faults {
        store: bytes.clone(),
        writes: Mutex::new(VecDeque::new()),
    });
    let (invocation, input, output, lease) = renew();
    let journal = Arc::new(
        Journal::initialize(
            invocation.provider,
            faults.clone(),
            Arc::new(CryptoPort),
            image_limits(),
            Anchor::new(time(0)),
        )
        .unwrap(),
    );
    let mut upload = bytes.begin(lease.id, 3, 3).unwrap();
    upload.append(0, b"abc").unwrap();
    drop(upload.finish(HASH).unwrap());
    let key = crate::modules::services::contract::OperationKey {
        stream: 1,
        ..invocation.key
    };
    let effect = Blob::new(Effect::Create(lease).encode(image_limits()).unwrap(), 65536).unwrap();
    let completion = Completion {
        output: Value::Uint64(17),
        result: Blob::new(vec![1], 1).unwrap(),
        price: 17,
        effect: effect.clone(),
    };
    journal.stage(key, [1; 32], completion).unwrap();
    let token = journal.prepare(key, &effect).unwrap();
    journal.commit(key, &token).unwrap();
    let gate = Gate::new(journal.clone(), bytes.clone(), limits()).unwrap();
    gate.recover(&Terminal(&journal), time(1), None).unwrap();
    Fixture {
        path,
        journal,
        bytes,
        faults,
        gate,
        invocation,
        input,
        output,
    }
}
