//! Metadata refusal is independent of execution and accounting.
use super::{
    Error,
    assessment::assess,
    authority::{Action, Authority},
    authorization::authorize,
    contract::{Context, Limits, OperationKey},
    handler::Peer,
    test_support::selection::selected,
    value::Value,
};
use crate::modules::identity::api::IdentityId;
use std::time::{Duration, Instant};
// Provider assessment retains exact terms and factors; malformed metadata never reaches execution.
#[test]
fn metadata_before_body() {
    let (_, s, _, r) = selected(false);
    let local = IdentityId([2; 32]);
    let a = Authority::new();
    let g = a.grant(local, Action::Invoke, 65536).unwrap();
    let c = Context {
        version: 1,
        grant: &g,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled: false,
    };
    let peer = Peer {
        local,
        remote: IdentityId([1; 32]),
        key: OperationKey {
            requester: s.roles().0,
            connection: [9; 32],
            stream: 1,
        },
    };
    let auth = || authorize(&c, &a, local, &r, s.clone(), peer, None).unwrap();
    assert!(matches!(
        assess(&c, &a, auth(), &Value::Bool(false), Limits::default()),
        Err(Error::Invalid)
    ));
    assert!(matches!(
        assess(
            &c,
            &a,
            auth(),
            &Value::String("metadata".into()),
            Limits {
                state_bytes: 2,
                ..Limits::default()
            }
        ),
        Err(Error::Limit)
    ));
    let assessed = assess(
        &c,
        &a,
        auth(),
        &Value::String("metadata".into()),
        Limits::default(),
    )
    .unwrap();
    assert_eq!(
        super::pricing::quote(&assessed, 7),
        Ok(super::pricing::Price::Fixed(7))
    );
    assert_eq!(
        super::pricing::quote(&assessed, 6),
        Err(Error::Insufficient)
    );
    assert_eq!(assessed.input_bytes(), 128);
    assert_eq!(assessed.factors(), &Value::Object(vec![]));
    assert_eq!(assessed.authorized().key(), peer.key);
    let expired = Context {
        cancelled: true,
        ..c
    };
    assert!(matches!(
        assess(
            &expired,
            &a,
            auth(),
            &Value::String("x".into()),
            Limits::default()
        ),
        Err(Error::Cancelled)
    ));
}
