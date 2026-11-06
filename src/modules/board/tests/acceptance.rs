//! Pinned BD acceptance rows translated into public direct-call contract scenarios.
use super::scenarios::Known;
use super::{
    Error,
    authority::{Action, Authority},
    cache::{Cache, Provenance},
    config::Limits,
    contract::Context,
    definition::Definition,
    offering::Board,
    prepared::Prepared,
    selection::Request,
    state::Provider,
};
use crate::modules::identity::api::IdentityId;
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};

// Required root fields, numeric ranges and all explicit issuer aliases reject independently.
#[test]
fn bd02_bd03_bd05_rejection_rows() {
    let base: serde_json::Value =
        serde_json::from_str(&super::test_support::board(false, 7)).unwrap();
    for (field, values) in [
        (
            "version",
            vec![
                serde_json::json!(0),
                serde_json::json!(-1),
                serde_json::json!(1.5),
                serde_json::json!("1"),
                serde_json::json!(true),
                serde_json::Value::Null,
            ],
        ),
        (
            "providedServices",
            vec![
                serde_json::Value::Null,
                serde_json::json!({}),
                serde_json::json!(true),
            ],
        ),
        (
            "wantedServices",
            vec![serde_json::Value::Null, serde_json::json!({})],
        ),
    ] {
        for value in values {
            let mut bad = base.clone();
            bad[field] = value;
            assert!(Board::parse(bad.to_string().as_bytes(), Limits::default()).is_err());
        }
    }
    for token in ["self", "own", "remote", "bad"] {
        let mut bad = base.clone();
        bad["providedServices"][0]["creditIssuer"] = token.into();
        assert!(Board::parse(bad.to_string().as_bytes(), Limits::default()).is_err());
    }
    for number in [
        serde_json::json!(0),
        serde_json::json!(4294967296_u64),
        serde_json::json!(-1),
        serde_json::json!(1.5),
    ] {
        let mut bad = base.clone();
        bad["providedServices"][0]["serviceVersion"] = number;
        assert!(Board::parse(bad.to_string().as_bytes(), Limits::default()).is_err());
    }
    for (provided, wanted) in [(2, 0), (0, 2), (1, 1)] {
        let mut bad = base.clone();
        let o = base["providedServices"][0].clone();
        bad["providedServices"] = vec![o.clone(); provided].into();
        bad["wantedServices"] = vec![o; wanted].into();
        assert!(Board::parse(bad.to_string().as_bytes(), Limits::default()).is_err());
    }
}
// Distinct IDs share one version; array reordering changes no selected terms or implicit priority.
#[test]
fn bd01_bd03_bd10_multiple_ids_and_array_order() {
    let id = IdentityId([1; 32]);
    let a = Arc::new(Authority::new());
    let p = Provider::new(id, a.clone(), Limits::default(), Arc::new(Known)).unwrap();
    let pg = a.grant(id, Action::Publish, 1_000_000).unwrap();
    let pc = Context {
        grant: &pg,
        deadline: Instant::now() + Duration::from_secs(10),
        cancelled: false,
    };
    let sg = a.grant(id, Action::Select, 1_000_000).unwrap();
    let sc = Context { grant: &sg, ..pc };
    let mut b: serde_json::Value =
        serde_json::from_str(&super::test_support::board(false, 7)).unwrap();
    let mut other = b["providedServices"][0].clone();
    other["id"] = "second".into();
    other["price"] = 9.into();
    b["providedServices"].as_array_mut().unwrap().push(other);
    let mut d: serde_json::Value = serde_json::from_str(super::test_support::definition()).unwrap();
    d["id"] = "second".into();
    let defs = BTreeMap::from([
        (
            "fixture".into(),
            super::test_support::definition().as_bytes().to_vec(),
        ),
        ("second".into(), d.to_string().into_bytes()),
    ]);
    let snapshot = p
        .publish(
            &pc,
            Prepared::new(
                id,
                b.to_string().as_bytes(),
                &defs,
                Limits::default(),
                &Known,
            )
            .unwrap(),
        )
        .unwrap();
    let r = Request {
        id: "fixture",
        service: "org.example.echo",
        version: 1,
        description: "Return the supplied label.",
        other: IdentityId([2; 32]),
        authenticated: true,
    };
    let selected = p.select(&sc, &snapshot, &r).unwrap();
    b["providedServices"].as_array_mut().unwrap().reverse();
    p.publish(
        &pc,
        Prepared::new(
            id,
            b.to_string().as_bytes(),
            &defs,
            Limits::default(),
            &Known,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(p.current_selection(&sc, &selected), Ok(()));
    let one = Limits {
        definitions: 1,
        ..Limits::default()
    };
    assert!(matches!(
        Board::parse(b.to_string().as_bytes(), one),
        Err(Error::Limit)
    ));
}
// An unknown sibling never hides known terms; mismatching or removed definitions never substitute versions.
#[test]
fn bd06_bd07_unknown_siblings_and_mismatches() {
    let id = IdentityId([1; 32]);
    let a = Arc::new(Authority::new());
    let mut cache = Cache::new(a.clone(), Limits::default()).unwrap();
    let g = a.grant(id, Action::Cache, 1_000_000).unwrap();
    let c = Context {
        grant: &g,
        deadline: Instant::now() + Duration::from_secs(10),
        cancelled: false,
    };
    let sg = a.grant(id, Action::Select, 1_000_000).unwrap();
    let sc = Context { grant: &sg, ..c };
    let mut b: serde_json::Value =
        serde_json::from_str(&super::test_support::board(false, 7)).unwrap();
    let mut unknown = b["providedServices"][0].clone();
    unknown["id"] = "unknown".into();
    unknown["service"] = "cr2se.future".into();
    unknown.as_object_mut().unwrap().remove("price");
    unknown["pricing"] = serde_json::json!({"model":"future.model"});
    unknown["preconditions"] = serde_json::json!(["future.condition"]);
    b["providedServices"].as_array_mut().unwrap().push(unknown);
    let remote = cache
        .store(&c, Provenance::Direct(id), b.to_string().as_bytes(), 100)
        .unwrap();
    let remote = cache
        .attach(
            &c,
            &remote,
            "fixture",
            super::test_support::definition().as_bytes(),
            100,
        )
        .unwrap();
    let request = Request {
        id: "fixture",
        service: "org.example.echo",
        version: 1,
        description: "Return the supplied label.",
        other: IdentityId([2; 32]),
        authenticated: true,
    };
    assert!(cache.select(&sc, &remote, &request, &Known, 101).is_ok());
    let board = Board::parse(remote.bytes(), Limits::default()).unwrap();
    let o = &board.offerings["fixture"];
    let mut d: serde_json::Value = serde_json::from_str(super::test_support::definition()).unwrap();
    d["serviceVersion"] = 2.into();
    assert!(Definition::parse(d.to_string().as_bytes(), o, Limits::default()).is_err());
    let mut d: serde_json::Value = serde_json::from_str(super::test_support::definition()).unwrap();
    d["id"] = "unknown".into();
    d["service"] = "cr2se.future".into();
    let remote = cache
        .attach(&c, &remote, "unknown", d.to_string().as_bytes(), 100)
        .unwrap();
    let unsupported = Request {
        id: "unknown",
        service: "cr2se.future",
        ..request
    };
    assert!(matches!(
        cache.select(&sc, &remote, &unsupported, &Known, 101),
        Err(Error::Unsupported)
    ));
}

// checkPrice has the same complete positive exact uint64 domain as the main amount.
#[test]
fn bd04_check_price_matrix() {
    let base: serde_json::Value =
        serde_json::from_str(&super::test_support::board(false, 7)).unwrap();
    for token in ["1", "18446744073709551615", "1.0", "10e-1"] {
        let mut v = base.clone();
        v["providedServices"][0]["checkPrice"] = serde_json::from_str(token).unwrap();
        assert!(Board::parse(v.to_string().as_bytes(), Limits::default()).is_ok());
    }
    for token in [
        "0",
        "-1",
        "18446744073709551616",
        "1.5",
        "\"7\"",
        "true",
        "null",
    ] {
        let mut v = base.clone();
        v["providedServices"][0]["checkPrice"] = serde_json::from_str(token).unwrap();
        assert!(Board::parse(v.to_string().as_bytes(), Limits::default()).is_err());
    }
}
