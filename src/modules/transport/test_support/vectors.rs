//! Loads fixed upstream bytes, without implementing framing or cryptography.
//! Provenance and intended use are recorded in fixtures/README.md.

const DOCUMENT: &str = include_str!("fixtures/transport-v1.md");

/// Occurrences are zero-based because PING and PONG share some upstream labels.
pub fn bytes(label: &str, occurrence: usize) -> Vec<u8> {
    let label = format!("{label}:");
    let lines: Vec<_> = DOCUMENT.lines().collect();
    let index = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| **line == label)
        .nth(occurrence)
        .unwrap_or_else(|| panic!("missing fixture {label} occurrence {occurrence}"))
        .0;
    decode_hex(lines.get(index + 1).expect("missing fixture bytes"))
}

pub fn array<const N: usize>(label: &str) -> [u8; N] {
    bytes(label, 0)
        .try_into()
        .unwrap_or_else(|_| panic!("wrong fixture length for {label}, expected {N}"))
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert!(value.len().is_multiple_of(2), "odd fixture hex length");
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            fn digit(byte: u8) -> u8 {
                match byte {
                    b'0'..=b'9' => byte - b'0',
                    b'a'..=b'f' => byte - b'a' + 10,
                    _ => panic!("invalid fixture hex"),
                }
            }
            digit(pair[0]) * 16 + digit(pair[1])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expected_upstream_material_has_fixed_lengths() {
        for label in ["initiator HELLO", "acceptor HELLO"] {
            let _: [u8; 100] = array(label);
        }
        for label in ["initiator AUTH payload", "acceptor AUTH payload"] {
            let _: [u8; 64] = array(label);
        }
        for label in [
            "initiator Ed25519 seed",
            "acceptor Ed25519 seed",
            "initiator raw X25519 private-key input",
            "acceptor raw X25519 private-key input",
            "initiator HELLO nonce",
            "acceptor HELLO nonce",
            "initiator Ed25519 public key",
            "acceptor Ed25519 public key",
            "initiator ephemeral X25519 public key",
            "acceptor ephemeral X25519 public key",
            "initiator CR2SE ID",
            "acceptor CR2SE ID",
            "SHA-256(helloTranscript)",
            "X25519 shared secret",
            "HKDF salt",
            "HKDF extract PRK",
            "initiator-to-acceptor key",
            "acceptor-to-initiator key",
        ] {
            let _: [u8; 32] = array(label);
        }
        for label in [
            "initiator-to-acceptor nonce prefix",
            "acceptor-to-initiator nonce prefix",
        ] {
            let _: [u8; 16] = array(label);
        }
    }

    #[test]
    fn fixed_ping_and_pong_match_their_recorded_components() {
        for occurrence in 0..2 {
            let frame = bytes("complete bytes written to TCP", occurrence);
            let header = bytes("header", occurrence);
            let payload = bytes("payload", occurrence);
            let tag = bytes("integrity tag", occurrence);
            assert_eq!((header.len(), payload.len(), tag.len()), (12, 8, 16));
            assert_eq!(frame, [header, payload, tag].concat());
            assert_eq!(bytes("nonce prefix || uint64_be(0)", occurrence).len(), 24);
        }
        assert_ne!(
            bytes("complete bytes written to TCP", 0),
            bytes("complete bytes written to TCP", 1)
        );
    }

    #[test]
    #[should_panic(expected = "missing fixture")]
    fn missing_fixture_names_fail_loudly() {
        bytes("not a fixture", 0);
    }

    #[test]
    #[should_panic(expected = "wrong fixture length")]
    fn wrong_array_width_is_not_truncated() {
        let _: [u8; 99] = array("initiator HELLO");
    }

    #[test]
    #[should_panic(expected = "invalid fixture hex")]
    fn invalid_hex_is_not_normalized() {
        decode_hex("0g");
    }

    #[test]
    #[should_panic(expected = "odd fixture hex length")]
    fn incomplete_hex_bytes_are_rejected() {
        decode_hex("abc");
    }
}
