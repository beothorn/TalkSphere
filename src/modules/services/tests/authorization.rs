//! Wanted input authority and application roles are independent of network direction.
use super::{
    Error,
    authority::{Action, Authority},
    authorization::authorize,
    contract::{Context, OperationKey},
    handler::Peer,
    test_support::selection::selected,
    value::Value,
};
use crate::modules::identity::api::IdentityId;
use std::time::{Duration, Instant};
// The publisher pays for wanted work only with its own explicit input and Request grant.
#[test]
fn wanted_and_provided_authority() {
    let local = IdentityId([2; 32]);
    let remote = IdentityId([1; 32]);
    for wanted in [false, true] {
        let (_, s, _, r) = selected(wanted);
        let a = Authority::new();
        let action = if wanted {
            Action::Request
        } else {
            Action::Invoke
        };
        let g = a.grant(local, action, 65536).unwrap();
        let c = Context {
            version: 1,
            grant: &g,
            deadline: Instant::now() + Duration::from_secs(60),
            cancelled: false,
        };
        let key = OperationKey {
            requester: s.roles().0,
            connection: [9; 32],
            stream: 1,
        };
        let peer = Peer { local, remote, key };
        if wanted {
            assert!(matches!(
                authorize(&c, &a, local, &r, s.clone(), peer, None),
                Err(Error::Denied)
            ));
        }
        let authorized = authorize(
            &c,
            &a,
            local,
            &r,
            s.clone(),
            peer,
            Some(Value::String("local input".into())),
        )
        .unwrap();
        assert_eq!(authorized.key(), key);
        assert_eq!(authorized.selection().roles(), s.roles());
        assert!(authorized.input().is_some());
        let wrong = a
            .grant(
                local,
                if wanted {
                    Action::Invoke
                } else {
                    Action::Request
                },
                65536,
            )
            .unwrap();
        assert!(matches!(
            authorize(
                &Context { grant: &wrong, ..c },
                &a,
                local,
                &r,
                s.clone(),
                peer,
                Some(Value::String("fabricated".into()))
            ),
            Err(Error::Denied)
        ));
        assert!(matches!(
            authorize(
                &c,
                &a,
                local,
                &r,
                s,
                Peer {
                    remote: IdentityId([8; 32]),
                    ..peer
                },
                Some(Value::String("x".into()))
            ),
            Err(Error::Denied)
        ));
    }
}
