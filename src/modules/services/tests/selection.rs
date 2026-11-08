//! Board selection through the actual Services capability registry.
use super::{Error, test_support::selection::selected};
use crate::modules::{board, identity::api::IdentityId};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
// Both directions bind exact roles; removing a publication blocks new acceptance without changing retained terms.
#[test]
fn real_board_selection_and_staleness() {
    for wanted in [false, true] {
        let (p, s, a, r) = selected(wanted);
        let publisher = IdentityId([2; 32]);
        let other = IdentityId([1; 32]);
        assert_eq!(
            s.roles(),
            if wanted {
                (publisher, other, publisher)
            } else {
                (other, publisher, publisher)
            }
        );
        let g = a
            .grant(publisher, board::authority::Action::Select, 65536)
            .unwrap();
        let c = board::contract::Context {
            grant: &g,
            deadline: Instant::now() + Duration::from_secs(60),
            cancelled: false,
        };
        assert_eq!(p.current_selection(&c, &s), Ok(()));
        let pg = a
            .grant(publisher, board::authority::Action::Publish, 65536)
            .unwrap();
        let empty = board::prepared::Prepared::new(
            publisher,
            br#"{"version":1,"providedServices":[],"wantedServices":[]}"#,
            &BTreeMap::new(),
            Default::default(),
            r.as_ref(),
        )
        .unwrap();
        p.publish(&board::contract::Context { grant: &pg, ..c }, empty)
            .unwrap();
        assert_eq!(p.current_selection(&c, &s), Err(board::Error::Stale));
        assert_eq!(s.offering().source["price"], 7);
        assert!(matches!(
            r.select("missing", 1, true),
            Err(Error::Unsupported)
        ));
    }
}
