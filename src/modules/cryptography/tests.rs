//! Scaffold tests cover policy and local contracts without simulating cryptography.

use super::{
    api::{DecryptRequest, EncryptRequest, IdentityEncryption, RecipientCredential},
    config::{MINIMUM_OBJECT_BYTES, ResourceLimits},
    error::CryptoError,
    ports::Entropy,
};

// Boundary and independent budgets are preserved without allocating their sizes.
#[test]
fn policy_accepts_minimum_and_host_sized_budgets() {
    for (plaintext, object) in [(1, MINIMUM_OBJECT_BYTES), (usize::MAX, usize::MAX)] {
        let limits = ResourceLimits::new(plaintext, object).unwrap();
        assert_eq!(limits.maximum_plaintext_bytes(), plaintext);
        assert_eq!(limits.maximum_object_bytes(), object);
    }
}

// Invalid policies cannot construct a usable immutable configuration.
#[test]
fn policy_rejects_zero_plaintext_and_objects_without_room_for_a_tag() {
    for (plaintext, object) in [(0, 148), (0, 0), (1, 0), (1, 132), (1, 147)] {
        assert_eq!(
            ResourceLimits::new(plaintext, object),
            Err(CryptoError::InvalidInput)
        );
    }
}

// Every error remains distinguishable while displaying only a fixed safe message.
#[test]
fn error_categories_have_fixed_nonempty_diagnostics() {
    let errors = [
        (CryptoError::InvalidInput, "invalid cryptographic input"),
        (
            CryptoError::AuthenticationFailed,
            "cryptographic authentication failed",
        ),
        (CryptoError::AccessDenied, "cryptographic key access denied"),
        (
            CryptoError::Unavailable,
            "cryptographic dependency unavailable",
        ),
        (
            CryptoError::ResourceLimit,
            "cryptographic resource limit exceeded",
        ),
    ];
    for (error, message) in errors {
        assert_eq!(error.to_string(), message);
        assert!(std::error::Error::source(&error).is_none());
    }
}

struct UnavailableEntropy;

impl Entropy for UnavailableEntropy {
    // This local fixture rejects entropy requests rather than generating fake keys.
    fn fill(&mut self, _output: &mut [u8]) -> Result<(), CryptoError> {
        Err(CryptoError::Unavailable)
    }
}

// Composition can substitute a failing entropy dependency and receive its error.
#[test]
fn entropy_dependency_is_fallible_and_object_safe() {
    let source: &mut dyn Entropy = &mut UnavailableEntropy;
    assert_eq!(source.fill(&mut [0; 32]), Err(CryptoError::Unavailable));
}

// A borrowed capability needs neither Debug nor Clone nor private-byte extraction.
struct LocalAuthority;
struct UnavailableEnvelopes;

impl IdentityEncryption for UnavailableEnvelopes {
    type PersistentKey = LocalAuthority;

    // An absent implementation returns no fabricated ciphertext, even for empty input.
    fn encrypt(&mut self, _request: EncryptRequest<'_>) -> Result<Vec<u8>, CryptoError> {
        Err(CryptoError::Unavailable)
    }

    // An absent implementation never mistakes input bytes for authenticated plaintext.
    fn decrypt(
        &self,
        _request: DecryptRequest<'_, Self::PersistentKey>,
    ) -> Result<Vec<u8>, CryptoError> {
        Err(CryptoError::Unavailable)
    }
}

// Real callers can construct borrowed requests without granting authority via public bytes.
#[test]
fn unavailable_envelope_boundary_returns_no_output_and_keeps_local_key_owned() {
    let limits = ResourceLimits::new(1024, 1172).unwrap();
    let recipient = RecipientCredential {
        identity_id: [1; 32],
        signing_public_key: [2; 32],
        encryption_public_key: [3; 32],
        binding_signature: [4; 64],
    };
    let mut envelopes = UnavailableEnvelopes;
    let key = LocalAuthority;
    assert_eq!(
        envelopes.encrypt(EncryptRequest {
            plaintext: b"",
            recipient: &recipient,
            limits
        }),
        Err(CryptoError::Unavailable)
    );
    for _ in 0..2 {
        assert_eq!(
            envelopes.decrypt(DecryptRequest {
                object: b"not an authenticated object",
                identity_id: recipient.identity_id,
                encryption_public_key: recipient.encryption_public_key,
                private_key: &key,
                limits,
            }),
            Err(CryptoError::Unavailable)
        );
    }
}
