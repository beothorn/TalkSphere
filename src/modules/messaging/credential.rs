//! The optional Board `identityEncryption` credential: how a sender finds a recipient's encryption key.
//!
//! Callers: a sender before encrypting (parse a recipient's Board), and a node publishing its own key. The
//! field is `{"version":1,"identityPublicKey","x25519PublicKey","bindingSignature"}` with unpadded RFC 4648
//! Base32 strings. A key is usable only when the ID derived from `identityPublicKey` is the expected Board
//! publisher, the binding signature verifies over the key, and the X25519 key is contributory. An invalid
//! field makes that key unusable and nothing else: the Board's offerings stay valid, and an opaque message
//! needs no credential at all. An unverified key found on a Board is never used.
use super::error::Error;
use crate::modules::{
    cryptography::{api::RecipientCredential, credential},
    identity::api::IdentityId,
};
use serde_json::{Value, json};

/// A verified recipient encryption key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecipientKey {
    /// The recipient's identity (the Board publisher).
    pub identity_id: IdentityId,
    /// The recipient's Ed25519 public key.
    pub identity_public_key: [u8; 32],
    /// The verified persistent X25519 public key to encrypt to.
    pub x25519_public_key: [u8; 32],
}

const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// Encode bytes as unpadded uppercase Base32.
pub fn base32_encode(bytes: &[u8]) -> String {
    tracing::trace!("Encoding Base32 text");
    let mut out = String::with_capacity(bytes.len().div_ceil(5) * 8);
    let (mut buffer, mut bits) = (0u32, 0u32);
    for byte in bytes {
        buffer = (buffer << 8) | u32::from(*byte);
        bits += 8;
        while bits >= 5 {
            out.push(ALPHABET[((buffer >> (bits - 5)) & 31) as usize] as char);
            bits -= 5;
        }
        buffer &= (1 << bits) - 1;
    }
    if bits > 0 {
        out.push(ALPHABET[((buffer << (5 - bits)) & 31) as usize] as char);
    }
    out
}

/// Decode strict unpadded RFC 4648 Base32: lowercase is accepted, but whitespace, padding, other characters and nonzero unused
/// trailing bits are not.
/// # Errors
/// `Invalid` for any of those.
pub fn base32_decode(text: &str) -> Result<Vec<u8>, Error> {
    tracing::trace!("Decoding Base32 text");
    let mut out = Vec::with_capacity(text.len() * 5 / 8);
    let (mut buffer, mut bits) = (0u32, 0u32);
    for character in text.bytes() {
        let value = match character.to_ascii_uppercase() {
            c @ b'A'..=b'Z' => c - b'A',
            c @ b'2'..=b'7' => c - b'2' + 26,
            _ => return Err(Error::Invalid),
        };
        buffer = (buffer << 5) | u32::from(value);
        bits += 5;
        if bits >= 8 {
            out.push((buffer >> (bits - 8)) as u8);
            bits -= 8;
            buffer &= (1 << bits) - 1;
        }
    }
    // Unused trailing bits must be zero, and a trailing partial group of five bits or more is not a valid length.
    if bits >= 5 || buffer != 0 {
        return Err(Error::Invalid);
    }
    Ok(out)
}

/// The `identityEncryption` member a node publishes for its own keys.
pub fn field_value(
    identity_public_key: &[u8; 32],
    x25519_public_key: &[u8; 32],
    binding_signature: &[u8; 64],
) -> Value {
    tracing::trace!("Building the identityEncryption Board member");
    json!({
        "version": 1,
        "identityPublicKey": base32_encode(identity_public_key),
        "x25519PublicKey": base32_encode(x25519_public_key),
        "bindingSignature": base32_encode(binding_signature),
    })
}

/// Look up and verify a recipient's key in a Board document published by `publisher`.
/// Returns `Ok(None)` when the Board has no `identityEncryption` member (the sender cannot use this lookup path; opaque messages still work).
/// # Errors
/// `Invalid` when the member is present but unusable: wrong shape or version, bad Base32 or lengths, an identity that is not the
/// publisher, a failed binding signature or a non-contributory X25519 key.
pub fn find_recipient_key(
    board: &Value,
    publisher: IdentityId,
) -> Result<Option<RecipientKey>, Error> {
    tracing::trace!("Looking up a recipient encryption credential on a Board");
    let Some(field) = board.get("identityEncryption") else {
        return Ok(None);
    };
    let object = field.as_object().ok_or(Error::Invalid)?;
    if object.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(Error::Invalid);
    }
    let get = |name: &str, length: usize| -> Result<Vec<u8>, Error> {
        let bytes = base32_decode(
            object
                .get(name)
                .and_then(Value::as_str)
                .ok_or(Error::Invalid)?,
        )?;
        (bytes.len() == length)
            .then_some(bytes)
            .ok_or(Error::Invalid)
    };
    let identity_key = get("identityPublicKey", 32)?;
    let x25519 = get("x25519PublicKey", 32)?;
    let binding = get("bindingSignature", 64)?;
    let credential = RecipientCredential {
        identity_id: publisher.0,
        signing_public_key: identity_key
            .clone()
            .try_into()
            .map_err(|_| Error::Invalid)?,
        encryption_public_key: x25519.clone().try_into().map_err(|_| Error::Invalid)?,
        binding_signature: binding.try_into().map_err(|_| Error::Invalid)?,
    };
    // The ID is derived from the key and must be the publisher's; the binding must verify over the X25519 key.
    credential::validate(&credential).map_err(|_| Error::Invalid)?;
    if !is_contributory(&credential.encryption_public_key) {
        return Err(Error::Invalid);
    }
    Ok(Some(RecipientKey {
        identity_id: publisher,
        identity_public_key: credential.signing_public_key,
        x25519_public_key: credential.encryption_public_key,
    }))
}

// Whether agreement with this key yields a nonzero shared secret (rejects every low-order point).
fn is_contributory(public_key: &[u8; 32]) -> bool {
    let secret = x25519_dalek::StaticSecret::from([9u8; 32]);
    secret
        .diffie_hellman(&x25519_dalek::PublicKey::from(*public_key))
        .was_contributory()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::{
        board::{config::Limits, offering::Board},
        messaging::{ports::Signer, test_support::TestIdentity},
    };

    // A recipient identity (seed 3) with a persistent X25519 key derived from its own seed.
    fn recipient() -> (TestIdentity, [u8; 32]) {
        let x25519 = x25519_dalek::PublicKey::from(&x25519_dalek::StaticSecret::from([0x60u8; 32]))
            .to_bytes();
        (TestIdentity::new(3), x25519)
    }
    // A Board with one Storage-free empty offering list and the given credential member.
    fn board(field: Value) -> Value {
        json!({"version": 1, "providedServices": [], "wantedServices": [], "identityEncryption": field})
    }
    fn good_field() -> (IdentityId, Value) {
        let (id, x25519) = recipient();
        let binding = id.bind_encryption_key(&x25519);
        (id.identity(), field_value(&id.key(), &x25519, &binding))
    }

    // Base32 matches RFC 4648 test vectors, round trips, and accepts lowercase.
    #[test]
    fn base32_matches_rfc_4648() {
        for (plain, encoded) in [
            ("", ""),
            ("f", "MY"),
            ("fo", "MZXQ"),
            ("foo", "MZXW6"),
            ("foob", "MZXW6YQ"),
            ("fooba", "MZXW6YTB"),
            ("foobar", "MZXW6YTBOI"),
        ] {
            assert_eq!(base32_encode(plain.as_bytes()), encoded);
            assert_eq!(base32_decode(encoded).unwrap(), plain.as_bytes());
            assert_eq!(
                base32_decode(&encoded.to_lowercase()).unwrap(),
                plain.as_bytes()
            );
        }
        let bytes: Vec<u8> = (0..=255).collect();
        assert_eq!(base32_decode(&base32_encode(&bytes)).unwrap(), bytes);
    }

    // Whitespace, padding, other characters, nonzero unused bits and impossible lengths are all refused.
    #[test]
    fn base32_is_strict() {
        for bad in [
            "MZ XW6",
            "MZXW6===",
            "MZXW6=",
            "MZXW1",
            "MZXW6\n",
            "MZXW6YTBOI ",
            "M",
            "MZX",
            "MZXW6YTBOJ",
            "MZXR",
        ] {
            assert_eq!(base32_decode(bad), Err(Error::Invalid), "{bad:?}");
        }
        // The last character carries four unused bits that must be zero: `MZXQ` decodes, `MZXR` does not.
        assert!(base32_decode("MZXQ").is_ok());
    }

    // MS-15: a canonical credential from the expected publisher is usable only after ID derivation, binding and X25519 validation.
    #[test]
    fn a_valid_credential_from_the_publisher_is_usable() {
        let (publisher, field) = good_field();
        let key = find_recipient_key(&board(field), publisher)
            .unwrap()
            .unwrap();
        assert_eq!(key.identity_id, publisher);
        assert_eq!(key.x25519_public_key, recipient().1);
        assert_eq!(key.identity_public_key, recipient().0.key());
        // The member also survives Board validation untouched, since unknown top-level members are inert.
        let text = serde_json::to_vec(&board(good_field().1)).unwrap();
        assert!(Board::parse(&text, Limits::default()).is_ok());
    }

    // MS-15: a wrong publisher, a bad binding, bad Base32 unused bits, a missing field and a low-order key each make the key
    // unusable (or absent), while the Board's offerings stay valid.
    #[test]
    fn unusable_credentials_are_refused_without_harming_the_board() {
        let (publisher, field) = good_field();
        assert_eq!(
            find_recipient_key(&board(field.clone()), IdentityId([9; 32])),
            Err(Error::Invalid)
        );
        let edit = |name: &str, change: fn(String) -> String| {
            let mut f = field.clone();
            let text = f[name].as_str().unwrap().to_string();
            f[name] = Value::String(change(text));
            find_recipient_key(&board(f), publisher)
        };
        // A different binding signature (re-encoded flipped byte) fails verification.
        let flipped = |text: String| {
            let mut bytes = base32_decode(&text).unwrap();
            bytes[0] ^= 1;
            base32_encode(&bytes)
        };
        assert_eq!(edit("bindingSignature", flipped), Err(Error::Invalid));
        assert_eq!(edit("x25519PublicKey", flipped), Err(Error::Invalid));
        assert_eq!(edit("identityPublicKey", flipped), Err(Error::Invalid));
        // Nonzero unused trailing bits, padding, whitespace and wrong lengths.
        assert_eq!(
            edit("x25519PublicKey", |t| format!("{}=", t)),
            Err(Error::Invalid)
        );
        assert_eq!(
            edit("x25519PublicKey", |t| format!(" {t}")),
            Err(Error::Invalid)
        );
        assert_eq!(
            edit("x25519PublicKey", |t| t[..t.len() - 1].to_string()),
            Err(Error::Invalid)
        );
        let last_nonzero = |t: String| {
            let mut chars: Vec<char> = t.chars().collect();
            let last = chars.len() - 1;
            chars[last] = if chars[last] == 'A' { 'B' } else { 'A' };
            chars.into_iter().collect()
        };
        assert_eq!(edit("x25519PublicKey", last_nonzero), Err(Error::Invalid));
        // Wrong version, wrong shape, and a low-order key with a binding made over it.
        let mut versioned = field.clone();
        versioned["version"] = 2.into();
        assert_eq!(
            find_recipient_key(&board(versioned), publisher),
            Err(Error::Invalid)
        );
        assert_eq!(
            find_recipient_key(&board(Value::String("x".into())), publisher),
            Err(Error::Invalid)
        );
        let (id, _) = recipient();
        let zero = [0u8; 32];
        let low_order = field_value(&id.key(), &zero, &id.bind_encryption_key(&zero));
        assert_eq!(
            find_recipient_key(&board(low_order), publisher),
            Err(Error::Invalid)
        );
        // A missing member is simply not available; opaque messages still work and unrelated offerings parse.
        let absent = json!({"version": 1, "providedServices": [], "wantedServices": []});
        assert_eq!(find_recipient_key(&absent, publisher), Ok(None));
        let bad_board = serde_json::to_vec(&board(Value::String("garbage".into()))).unwrap();
        assert!(Board::parse(&bad_board, Limits::default()).is_ok());
    }

    // An opaque message needs no credential: it signs and verifies with none present.
    #[test]
    fn opaque_messages_need_no_credential() {
        use crate::modules::messaging::{ids::MessageId, signing, state::Encoding};
        let sender = TestIdentity::new(1);
        let signer: &dyn Signer = &sender;
        let message = signing::sign_message(
            signer,
            IdentityId([3; 32]),
            MessageId([1; 32]),
            1,
            Encoding::Opaque,
            b"hello".to_vec(),
        )
        .unwrap();
        assert_eq!(signing::verify_message(&message, None), Ok(()));
        assert_eq!(
            find_recipient_key(&json!({"version": 1}), IdentityId([3; 32])),
            Ok(None)
        );
    }
}
