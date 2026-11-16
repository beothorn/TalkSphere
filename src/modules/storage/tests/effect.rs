//! Exact proposal codec and immutable obligation guards, independent of participant persistence.
use super::*;
use crate::modules::services::{handler::Blob, value::Budget};
// Use complete owned lease metadata with independently fixed identities and times.
fn lease() -> Lease {
    Lease {
        id: [3; 32],
        requester: IdentityId([2; 32]),
        provider: IdentityId([1; 32]),
        accepted_terms: Blob::new(b"terms".to_vec(), 5).unwrap(),
        size_bytes: 3,
        content_hash: [4; 32],
        accepted_at: 1000,
        expires_at: 1010,
        store_price: 7,
        state: State::Active,
    }
}
// Bound private proposal data independently from peer offering limits.
fn limits() -> Limits {
    Limits {
        bytes: 4096,
        leases: 4,
        operations: 4,
        values: Budget {
            bytes: 128,
            nodes: 16,
            depth: 4,
        },
    }
}
// Start with no leases; proposal application changes only this test-owned candidate image.
fn image() -> Image {
    Image {
        provider: IdentityId([1; 32]),
        anchor: Anchor::new(Time {
            unix_seconds: 1000,
            elapsed_seconds: 0,
            boot: [1; 16],
        }),
        leases: vec![],
        operations: vec![],
    }
}
// Every proposal round-trips without extra effects, and malformed/trailing private encodings are rejected.
#[test]
fn exact_proposal_formats() {
    for effect in [
        Effect::NoChange,
        Effect::Create(lease()),
        Effect::Renew {
            expected_expires_at: 1009,
            lease: lease(),
        },
        Effect::Remove {
            id: [3; 32],
            expected_expires_at: 1010,
        },
    ] {
        let bytes = effect.encode(limits()).unwrap();
        let decoded = Effect::decode(&bytes, IdentityId([1; 32]), limits()).unwrap();
        assert_eq!(decoded.encode(limits()).unwrap(), bytes);
        assert_eq!(decoded.resource(), effect.resource());
        for end in 0..bytes.len() {
            assert!(Effect::decode(&bytes[..end], IdentityId([1; 32]), limits()).is_err());
        }
        let mut extra = bytes.clone();
        extra.push(0);
        assert!(Effect::decode(&extra, IdentityId([1; 32]), limits()).is_err());
        assert!(matches!(
            Effect::decode(
                &bytes,
                IdentityId([1; 32]),
                Limits {
                    bytes: 0,
                    ..limits()
                }
            ),
            Err(Error::Limit)
        ));
    }
    assert!(matches!(
        Effect::decode(&[9], IdentityId([1; 32]), limits()),
        Err(Error::Invalid)
    ));
    assert!(matches!(
        Effect::Create(lease()).encode(Limits {
            bytes: 128,
            ..limits()
        }),
        Err(Error::Limit)
    ));
    assert!(matches!(
        Effect::decode(
            &Effect::Create(lease()).encode(limits()).unwrap(),
            IdentityId([9; 32]),
            limits()
        ),
        Err(Error::Denied)
    ));
}
// Create and removal enforce authenticated ownership and retain terminal ID history rather than permitting reuse.
#[test]
fn owned_create_and_removal() {
    let mut state = image();
    let l = lease();
    let owner = l.requester;
    let create = Effect::Create(l.clone());
    assert_eq!(
        create.apply(&mut state, IdentityId([9; 32])),
        Err(Error::Denied)
    );
    assert!(state.leases.is_empty());
    create.apply(&mut state, owner).unwrap();
    assert_eq!(create.apply(&mut state, owner), Err(Error::Conflict));
    let remove = Effect::Remove {
        id: l.id,
        expected_expires_at: 1010,
    };
    assert_eq!(
        remove.apply(&mut state, IdentityId([9; 32])),
        Err(Error::Denied)
    );
    assert_eq!(
        Effect::Remove {
            id: [9; 32],
            expected_expires_at: 1010
        }
        .apply(&mut state, owner),
        Err(Error::Denied)
    );
    assert_eq!(
        Effect::Remove {
            id: l.id,
            expected_expires_at: 1009
        }
        .apply(&mut state, owner),
        Err(Error::Conflict)
    );
    remove.apply(&mut state, owner).unwrap();
    assert_eq!(state.leases[0].state, State::Removed);
    assert_eq!(remove.apply(&mut state, owner), Err(Error::Conflict));
    assert_eq!(create.apply(&mut state, owner), Err(Error::Conflict));
    assert_eq!(Effect::NoChange.apply(&mut state, owner), Ok(()));
}
// Every field except expiration is immutable on renewal; stale, inactive and foreign requests leave it untouched.
#[test]
fn renewal_preserves_original_obligation() {
    let mut state = image();
    let original = lease();
    Effect::Create(original.clone())
        .apply(&mut state, original.requester)
        .unwrap();
    for field in 0..10 {
        let mut changed = original.clone();
        changed.expires_at = 1014;
        match field {
            0 => changed.requester = IdentityId([9; 32]),
            1 => changed.provider = IdentityId([9; 32]),
            2 => changed.accepted_terms = Blob::new(vec![0], 1).unwrap(),
            3 => changed.size_bytes = 4,
            4 => changed.content_hash = [9; 32],
            5 => changed.accepted_at = 999,
            6 => changed.store_price = 8,
            7 => changed.state = State::Removed,
            8 => changed.expires_at = 1010,
            _ => changed.expires_at = 1009,
        }
        assert!(
            Effect::Renew {
                expected_expires_at: 1010,
                lease: changed
            }
            .apply(&mut state, original.requester)
            .is_err()
        );
        assert_eq!(state.leases[0].expires_at, 1010);
    }
    let mut renewed = original.clone();
    renewed.expires_at = 1014;
    let effect = Effect::Renew {
        expected_expires_at: 1010,
        lease: renewed,
    };
    effect.apply(&mut state, original.requester).unwrap();
    assert_eq!(state.leases[0].expires_at, 1014);
    assert_eq!(
        effect.apply(&mut state, original.requester),
        Err(Error::Conflict)
    );
    state.leases[0].state = State::Expired;
    assert_eq!(
        Effect::Remove {
            id: original.id,
            expected_expires_at: 1014
        }
        .apply(&mut state, original.requester),
        Err(Error::Conflict)
    );
}
