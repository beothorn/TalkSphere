//! OPEN wire and malformed-input tests for TR-08.
use super::*;

// Independent bytes establish byte order and field widths rather than a round trip alone.
#[test]
fn exact_layout_and_namespace_boundaries() {
    let bytes = b"\0\x0acr2se.peer\0\0\0\x01";
    let open = Open::decode(bytes).unwrap();
    assert_eq!(open.id(), "cr2se.peer");
    assert_eq!(open.version(), 1);
    assert_eq!(open.encode(), bytes);
    for id in ["a", "0", "a-b.c", "a.-b", "a--b", "org.example.thumbnail"] {
        assert_eq!(
            Open::decode(&Open::new(id, u32::MAX).unwrap().encode())
                .unwrap()
                .id(),
            id
        );
    }
    let max = "a".repeat(255);
    assert_eq!(Open::new(&max, 1).unwrap().encode().len(), 261);
    assert_eq!(Open::new(&"a".repeat(256), 1), Err(OpenError::Length));
}

// Every malformed namespace and reserved version fails independently of registry support.
#[test]
fn invalid_identifiers_and_versions() {
    assert_eq!(Open::new("", 1), Err(OpenError::Length));
    for id in [
        "A", "a..b", ".a", "a.", "-a", "a-", "a_b", "a b", "é", "a\0b",
    ] {
        assert_eq!(Open::new(id, 1), Err(OpenError::Identifier), "{id:?}");
    }
    assert_eq!(Open::new("unknown.valid", 0), Err(OpenError::Version));
    assert!(Open::new("unknown.valid", 1).is_ok());
    assert_eq!(
        Open::decode(&[0, 1, 255, 0, 0, 0, 1]),
        Err(OpenError::Identifier)
    );
}

// Exact widths reject every truncated prefix, trailing bytes and misleading lengths.
#[test]
fn malformed_lengths_and_encoded_zero_version() {
    let valid = Open::new("cr2se.peer", 1).unwrap().encode();
    for end in 0..valid.len() {
        assert_eq!(Open::decode(&valid[..end]), Err(OpenError::Length));
    }
    let mut bytes = valid.clone();
    bytes.push(0);
    assert_eq!(Open::decode(&bytes), Err(OpenError::Length));
    for length in [0u16, 1, 255, 256, u16::MAX] {
        let mut bytes = valid.clone();
        bytes[..2].copy_from_slice(&length.to_be_bytes());
        assert_eq!(Open::decode(&bytes), Err(OpenError::Length));
    }
    let mut bytes = valid;
    *bytes.last_mut().unwrap() = 0;
    assert_eq!(Open::decode(&bytes), Err(OpenError::Version));
}
