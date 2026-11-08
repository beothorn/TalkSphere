//! Local volatile Services demonstration over real Board and Ledger public contracts.
#[path = "services/family.rs"]
mod family;
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};
use talksphere::modules::{
    board,
    identity::{api::IdentityId, text},
    ledger,
    services::{
        self,
        authority::{Action, Authority, Grant},
        contract::{Context, Limits, OperationKey},
        handler::{Blob, Peer, Requester},
        runtime::{Config, Coordinator, Payment},
        value::Value,
    },
};

// Render typed contract failures without exposing application values or private state.
fn failure<E: std::fmt::Debug>(error: E) -> String {
    format!("Local Services demonstration failed: {error:?}")
}
// Short-lived contexts originate in local composition and are never deserialized from peer bytes.
fn context(grant: &Grant) -> Context<'_> {
    tracing::trace!("Creating example Services context");
    Context {
        version: 1,
        grant,
        deadline: Instant::now() + Duration::from_secs(30),
        cancelled: false,
    }
}
// All stores and the custom handler are volatile; every invocation of this example starts a new demonstration.
fn exercise() -> Result<(u64, u64), String> {
    tracing::trace!("Composing explicit finite local Services demonstration");
    let local = IdentityId([2; 32]);
    let requester = IdentityId([1; 32]);
    let authority = Arc::new(Authority::new());
    let init = authority
        .grant(local, Action::Register, 1_000_000)
        .map_err(failure)?;
    let coordinate = authority
        .grant(local, Action::Coordinate, 1_000_000)
        .map_err(failure)?;
    let observe = authority
        .grant(local, Action::Observe, 1_000_000)
        .map_err(failure)?;
    let invoke = authority
        .grant(local, Action::Invoke, 1_000_000)
        .map_err(failure)?;
    let limits = Limits {
        handlers: 1,
        operations: 1,
        bodies: 2,
        nodes: 4096,
        quotes: 1,
        state_bytes: 65536,
    };
    let registry = Arc::new(
        services::registry::Registry::new(
            local,
            authority.clone(),
            limits,
            Arc::new(services::registry::NoStandards),
        )
        .map_err(failure)?,
    );
    let (handler, offering, bytes) = family::Echo::new(text::format(local))?;
    let handler = Arc::new(handler);
    registry
        .register(&context(&init), handler.clone(), Some(handler.clone()))
        .map_err(failure)?;
    let ba = Arc::new(board::authority::Authority::new());
    let owner =
        board::state::Provider::new(local, ba.clone(), Default::default(), registry.clone())
            .map_err(failure)?;
    let publish = ba
        .grant(local, board::authority::Action::Publish, 1_000_000)
        .map_err(failure)?;
    let select = ba
        .grant(local, board::authority::Action::Select, 1_000_000)
        .map_err(failure)?;
    let bc = board::contract::Context {
        grant: &publish,
        deadline: context(&init).deadline,
        cancelled: false,
    };
    let candidate = board::prepared::Prepared::new(
        local,
        &bytes,
        &BTreeMap::from([(
            "echo".into(),
            handler.definition().document.bytes().to_vec(),
        )]),
        Default::default(),
        registry.as_ref(),
    )
    .map_err(failure)?;
    let snapshot = owner.publish(&bc, candidate).map_err(failure)?;
    let bc = board::contract::Context {
        grant: &select,
        ..bc
    };
    let selection = owner
        .select(
            &bc,
            &snapshot,
            &board::selection::Request {
                id: &offering.id,
                service: &offering.service,
                version: 1,
                description: &offering.description,
                other: requester,
                authenticated: true,
            },
        )
        .map_err(failure)?;
    let peer = Peer {
        local,
        remote: requester,
        key: OperationKey {
            requester,
            connection: [8; 32],
            stream: 1,
        },
    };
    let input = Value::String("hello Services".into());
    let auth = services::authorization::authorize(
        &context(&invoke),
        &authority,
        local,
        &registry,
        selection,
        peer,
        None,
    )
    .map_err(failure)?;
    let assessed =
        services::assessment::assess(&context(&invoke), &authority, auth, &input, limits)
            .map_err(failure)?;
    let quoter = services::quote::Quoter::new(local, authority.clone(), limits).map_err(failure)?;
    // Literal quote bytes are a trusted local exercise binding, not a Peer Protocol encoder.
    let quote = quoter
        .issue(
            &context(&invoke),
            services::quote::Issue {
                assessed,
                bytes: Blob::new(b"local-example-price7".to_vec(), 128).map_err(failure)?,
                maximum: 7,
                expires: bc.deadline,
            },
        )
        .map_err(failure)?;
    let accepted = quoter
        .accept(
            &context(&invoke),
            &quote,
            peer,
            quote.hash(),
            &services::quote::BoardCurrent {
                owner: &owner,
                context: &bc,
            },
        )
        .map_err(failure)?;
    let la = Arc::new(ledger::authority::Authority::new());
    let admin = la
        .grant(local, ledger::authority::Action::Administer, 1_000_000)
        .map_err(failure)?;
    let lg = ledger::contract::Context {
        version: 1,
        grant: &admin,
        deadline: bc.deadline,
        cancelled: false,
    };
    let ledger = Arc::new(
        ledger::state::Ledger::initialize(
            local,
            la.clone(),
            ledger::persistence::Memory::default(),
            Default::default(),
            &lg,
            1000,
            100,
        )
        .map_err(failure)?,
    );
    ledger.grant(&lg, requester, 100).map_err(failure)?;
    ledger
        .set_trust(
            &lg,
            requester,
            ledger::accounting::Trust::new(1).map_err(failure)?,
        )
        .map_err(failure)?;
    let grant = la
        .grant(local, ledger::authority::Action::Coordinate, 1_000_000)
        .map_err(failure)?;
    let accounting = Arc::new(services::accounting::Ledger::new(
        ledger.clone(),
        grant,
        Default::default(),
    ));
    let runtime = Coordinator::open(
        Config {
            local,
            authority,
            limits,
        },
        &context(&init),
        services::persistence::Memory::default(),
        accounting,
        registry,
        true,
    )
    .map_err(failure)?;
    let key = runtime
        .admit(&context(&coordinate), accepted, Payment::default())
        .map_err(failure)?;
    runtime
        .supply_input(&context(&coordinate), key, input)
        .map_err(failure)?;
    runtime.start(&context(&coordinate), key).map_err(failure)?;
    runtime
        .resume(&context(&coordinate), key)
        .map_err(failure)?;
    let result = runtime.result(&context(&observe), key).map_err(failure)?;
    let read = la
        .grant(local, ledger::authority::Action::Read, 1_000_000)
        .map_err(failure)?;
    let account = ledger
        .view(&ledger::contract::Context { grant: &read, ..lg }, requester)
        .map_err(failure)?;
    Ok((result.price, account.issued))
}
// Demonstration help is side-effect-free; invalid syntax exits two and runtime failure exits one.
fn main() {
    tracing::trace!("Starting local Services example");
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--help"] || args == ["-h"] {
        println!(
            "Usage: cargo run --locked --example services\nExplicit local volatile custom handler; price=7, remaining=93. No networking or restart persistence. Exit status: 0 success/help, 1 failure, 2 invalid arguments.\nSee talksphere help services and src/modules/services/README.md."
        );
        return;
    }
    if !args.is_empty() {
        eprintln!("Use --help for the local demonstration.");
        std::process::exit(2);
    }
    match exercise() {
        Ok((price, remaining)) => {
            println!("Local volatile Services: price={price}, remaining={remaining}")
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
#[cfg(test)]
mod tests {
    // The complete public-contract example settles exactly seven from a finite one-hundred credit grant.
    #[test]
    fn local_example_settles_once() {
        assert_eq!(super::exercise(), Ok((7, 93)));
    }
}
