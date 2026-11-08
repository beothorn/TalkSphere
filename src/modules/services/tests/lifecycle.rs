//! Registry draining stops new advertising before disabling code, while immutable old terms remain owned.
use super::{
    authority::{Action, Authority},
    contract::Context,
    test_support::{family::Echo, selection::selected_with_family},
};
use crate::{
    composition::services::Publication,
    modules::{board, identity::api::IdentityId},
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
// Application composition serializes publication pruning and registry draining without touching Board private tables.
#[test]
fn unpublish_before_drain() {
    let local = IdentityId([2; 32]);
    let a = Arc::new(Authority::new());
    let family = Arc::new(Echo::new());
    let (owner, selected, ba, registry) = selected_with_family(false, family, a.clone());
    let publication = Publication::new(local, owner, registry.clone(), Default::default()).unwrap();
    let pinned = registry.select("org.example.echo", 1, true).unwrap();
    let g = a.grant(local, Action::Register, 1_000_000).unwrap();
    let c = Context {
        version: 1,
        grant: &g,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    };
    let read = ba
        .grant(local, board::authority::Action::Read, 1_000_000)
        .unwrap();
    let publish = ba
        .grant(local, board::authority::Action::Publish, 1_000_000)
        .unwrap();
    let select = ba
        .grant(local, board::authority::Action::Select, 1_000_000)
        .unwrap();
    let bc = board::contract::Context {
        grant: &read,
        deadline: c.deadline,
        cancelled: false,
    };
    publication
        .drain(
            &c,
            &bc,
            &board::contract::Context {
                grant: &publish,
                ..bc
            },
            "org.example.echo",
            1,
        )
        .unwrap();
    let snapshot = publication.read(&bc).unwrap();
    let board = board::offering::Board::parse(snapshot.bytes(), Default::default()).unwrap();
    assert!(board.offerings.is_empty());
    assert!(registry.select("org.example.echo", 1, true).is_err());
    assert!(pinned.provider().is_ok());
    assert_eq!(selected.offering().source["price"], 7);
    assert!(
        publication
            .current(
                &board::contract::Context {
                    grant: &select,
                    ..bc
                },
                &selected
            )
            .is_err()
    );
}
