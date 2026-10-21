//! Observable scaffold boundaries; no fake credential may escape unfinished creation.
use super::{api::*, config::Limits, error::IdentityError};
struct Unavailable;
impl IdentityProvider for Unavailable {
    type Handle = ();
    type Administration = ();
}
// Invalid local budgets fail before any storage or cryptographic operation.
#[test]
fn budgets_are_independent_and_validate_minima() {
    for args in [
        (57, 1, 1, 148, 1),
        (58, 0, 1, 148, 1),
        (58, 1, 0, 148, 1),
        (58, 1, 1, 147, 1),
        (58, 1, 1, 148, 0),
    ] {
        assert_eq!(
            Limits::new(args.0, args.1, args.2, args.3, args.4),
            Err(IdentityError::InvalidInput)
        );
    }
    let limits = Limits::new(58, 2, 3, 148, 5).unwrap();
    assert_eq!(
        (
            limits.text_bytes(),
            limits.contacts(),
            limits.signing_bytes(),
            limits.envelope_bytes(),
            limits.record_bytes()
        ),
        (58, 2, 3, 148, 5)
    );
}
// Unsupported contracts and unfinished lifecycle calls cannot succeed with placeholders.
#[test]
fn unavailable_operations_and_versions_are_explicit() {
    assert_eq!(validate_contract(1), Ok(()));
    for version in [0, 2, u16::MAX] {
        assert_eq!(validate_contract(version), Err(IdentityError::Unsupported));
    }
    assert_eq!(Unavailable.create(&()), Err(IdentityError::Unavailable));
    assert_eq!(Unavailable.load(&()), Err(IdentityError::Unavailable));
}
// Public snapshots carry fixed public values, while diagnostics have no input fields.
#[test]
fn snapshots_and_fixed_errors_preserve_boundaries() {
    let identity = PublicIdentity {
        id: IdentityId([1; 32]),
        signing_public_key: [2; 32],
    };
    assert_eq!(identity.id(), IdentityId([1; 32]));
    assert_eq!(identity.signing_public_key(), [2; 32]);
    for error in [
        IdentityError::InvalidInput,
        IdentityError::Unsupported,
        IdentityError::IdentityMismatch,
        IdentityError::InvalidProof,
        IdentityError::AccessDenied,
        IdentityError::Unavailable,
        IdentityError::ResourceLimit,
        IdentityError::Conflict,
        IdentityError::OutcomeUnknown,
    ] {
        assert!(!error.to_string().is_empty());
        assert!(std::error::Error::source(&error).is_none());
    }
}
