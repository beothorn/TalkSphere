//! Requester recovery drops all module owners and reopens independent native journals; remote results and lease expectations remain controlled fixtures.
mod boundaries;
mod faults;
mod payment;
use super::*;
use services::runtime::{Decision, Phase};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};
type Native = Endpoint<ledger::linux::FileStore, services::linux::FileStore>;
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Directory(PathBuf);
impl Directory {
    // Allocate a unique protected parent for independently locked Services and Ledger state, never sharing files across scenarios.
    fn new() -> Self {
        use std::os::unix::fs::DirBuilderExt;
        let path = std::env::temp_dir().join(format!(
            "talksphere-storage-requester-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    // Dispose only this test's isolated state after owners have released their directory/file descriptors.
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
// Fresh codecs, Registry, grants and module heaps reopen acknowledged bytes from separate public native adapters.
fn open(path: &Path, inv: &Invocation, initialize: bool) -> Native {
    open_with_payment(path, inv, inv.provider, Payment::default(), initialize)
}
// Reopen the originally accepted currency/fee binding without substituting the provider or minting replacement funds.
fn open_with_payment(
    path: &Path,
    inv: &Invocation,
    issuer: IdentityId,
    payment: Payment,
    initialize: bool,
) -> Native {
    let f = fixture();
    endpoint_with_stores(
        f.client,
        inv,
        issuer,
        payment,
        ledger::linux::FileStore::open(&path.join("ledger"), Default::default(), initialize)
            .unwrap(),
        services::linux::FileStore::open(&path.join("services"), Default::default(), initialize)
            .unwrap(),
        initialize,
    )
}
// Bind an operation's original complete remote output to requester Services only after explicit original result.end and normal END.
fn stage<L: ledger::persistence::Persistence, S: services::persistence::Persistence>(
    endpoint: &Endpoint<L, S>,
    outgoing: &Outgoing,
    output: &Value,
    price: u64,
) {
    let (grant, mut receiver) = result::receiver(endpoint, outgoing);
    result::deliver(endpoint, &grant, &mut receiver, output, price, true);
    let coordinate = coordinate(endpoint);
    receiver
        .finish(
            &pc(&grant),
            endpoint.association,
            true,
            &sc(&coordinate),
            &endpoint.runtime,
        )
        .unwrap();
}
// Each reopened coordinator has a new authority domain; no pre-restart grant is reused to deliver decisions.
fn coordinate<L: ledger::persistence::Persistence, S: services::persistence::Persistence>(
    endpoint: &Endpoint<L, S>,
) -> services::authority::Grant {
    endpoint
        .services
        .grant(
            endpoint.association.local,
            services::authority::Action::Coordinate,
            16_777_216,
        )
        .unwrap()
}
// Read retained output only from the freshly opened owner under an independent Observe grant.
fn completed<L: ledger::persistence::Persistence, S: services::persistence::Persistence>(
    endpoint: &Endpoint<L, S>,
) -> Result<services::handler::Completion, Error> {
    let read = endpoint
        .services
        .grant(
            endpoint.association.local,
            services::authority::Action::Observe,
            16_777_216,
        )
        .unwrap();
    endpoint.runtime.result(&sc(&read), endpoint.key())
}
// Preserve every typed field while ignoring JSON object member ordering, which is not protocol meaning.
fn same_output(actual: Value, expected: &Value) {
    let (Value::Object(actual), Value::Object(expected)) = (actual, expected.clone()) else {
        panic!("Storage result");
    };
    assert_eq!(
        actual.into_iter().collect::<BTreeMap<_, _>>(),
        expected.into_iter().collect::<BTreeMap<_, _>>()
    );
}
