//! Authorized direct-call accounting walkthrough; public opaque IDs do not authenticate network peers.
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use talksphere::modules::{
    identity::api::IdentityId,
    ledger::{
        Error,
        authority::{Action, Authority},
        contract::{Context, Limits, OperationKey},
        persistence::Memory,
        state::Ledger,
        terms::{Policy, Terms},
    },
};
// Demonstrate one finite provider-credit payment and exact replay without files, keys or networking.
fn main() -> Result<(), Error> {
    tracing::trace!("Running the explicit volatile Ledger accounting example");
    let requester = IdentityId([1; 32]);
    let provider = IdentityId([2; 32]);
    let authority = Arc::new(Authority::new());
    let limits = Limits::default();
    let admin = authority.grant(requester, Action::Administer, limits.state_bytes)?;
    let read = authority.grant(requester, Action::Read, limits.state_bytes)?;
    let coordinate = authority.grant(requester, Action::Coordinate, limits.state_bytes)?;
    let deadline = Instant::now() + Duration::from_secs(30);
    let admin_call = Context {
        version: 1,
        grant: &admin,
        deadline,
        cancelled: false,
    };
    let read_call = Context {
        grant: &read,
        ..admin_call
    };
    let coordinator_call = Context {
        grant: &coordinate,
        ..admin_call
    };
    let ledger = Ledger::initialize(
        requester,
        authority,
        Memory::default(),
        limits,
        &admin_call,
        10,
        10,
    )?;
    ledger.set_owned(&admin_call, provider, 100)?;
    let key = OperationKey {
        requester,
        connection: [3; 32],
        stream: 1,
    };
    let reservation = ledger.reserve(
        &coordinator_call,
        key,
        Terms {
            requester,
            provider,
            issuer: provider,
            maximum: 7,
            fixed: true,
            accepted: b"explicit local example terms".to_vec(),
            transfer: None,
            fee: None,
            spending_bound: 7,
        },
        Policy::default(),
    )?;
    println!(
        "reserved: balance={}, available={}",
        ledger.view(&read_call, provider)?.owned,
        ledger.available(&read_call, provider, true)?
    );
    let prepared = ledger.prepare(
        &coordinator_call,
        &reservation,
        7,
        b"complete example result reference".to_vec(),
    )?;
    // This example acts as the authorized coordinator after its own complete-result validation.
    ledger.commit(&coordinator_call, &prepared)?;
    ledger.commit(&coordinator_call, &prepared)?;
    println!(
        "committed once: balance={}, available={}",
        ledger.view(&read_call, provider)?.owned,
        ledger.available(&read_call, provider, true)?
    );
    Ok(())
}
