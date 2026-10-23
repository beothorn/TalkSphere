//! Canonical RFC 4648 Base32 identity text; codecs never consult storage or networking.
use super::api::IdentityId;
const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// Format exactly 32 ID bytes as lowercase prefix plus 52 uppercase unpadded symbols.
/// The final symbol's unused bits are zero; no key or network operation occurs.
pub fn format(id: IdentityId) -> String {
    tracing::trace!("Formatting a canonical textual identity");
    let mut output = String::with_capacity(58);
    output.push_str("cr2se:");
    let mut accumulator = 0u16;
    let mut bits = 0;
    for byte in id.0 {
        accumulator = (accumulator << 8) | u16::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            output.push(char::from(
                ALPHABET[usize::from((accumulator >> bits) & 31)],
            ));
        }
        accumulator &= (1 << bits) - 1;
    }
    if bits != 0 {
        output.push(char::from(ALPHABET[usize::from(accumulator << (5 - bits))]));
    }
    output
}
/// Parse exactly one textual identity without trimming or repairing invalid input.
/// Only ASCII Base32 letters may vary in case; the prefix and unused bits are exact.
/// # Errors
/// Returns InvalidInput for wrong lengths, prefix, alphabet, padding or unused bits.
pub fn parse(text: &str) -> Result<IdentityId, super::error::IdentityError> {
    tracing::trace!("Parsing a strict textual identity");
    use super::error::IdentityError;
    if text.len() != 58 || !text.starts_with("cr2se:") {
        return Err(IdentityError::InvalidInput);
    }
    let mut bytes = [0u8; 32];
    let mut offset = 0;
    let mut accumulator = 0u16;
    let mut bits = 0;
    for byte in text.as_bytes()[6..].iter().copied() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a',
            b'2'..=b'7' => byte - b'2' + 26,
            _ => return Err(IdentityError::InvalidInput),
        };
        accumulator = (accumulator << 5) | u16::from(value);
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            bytes[offset] = (accumulator >> bits) as u8;
            offset += 1;
            accumulator &= (1 << bits) - 1;
        }
    }
    if accumulator != 0 {
        return Err(IdentityError::InvalidInput);
    }
    Ok(IdentityId(bytes))
}

/// Normalize accepted ASCII Base32 casing to the canonical display representation.
/// # Errors
/// Propagates strict parse rejection; no whitespace or invalid padding is repaired.
pub fn normalize(text: &str) -> Result<String, super::error::IdentityError> {
    tracing::trace!("Normalizing an accepted textual identity");
    parse(text).map(format)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::identity::test_support::{TRANSPORT, vector};
    // Both upstream textual forms and extreme bit patterns fix exact alphabet, padding and casing.
    #[test]
    fn fixed_text_and_bit_boundaries_are_canonical() {
        for (role, text) in [
            (
                "initiator",
                "cr2se:3FEKTKCHTGJQLPTATWZZ3HFFBN6JGGCOYNV56CAPN5DECXQZSJOQ",
            ),
            (
                "acceptor",
                "cr2se:O5CQJ3PUPOXCRJHRCVF7IULK45HQSDXFWAZEMRMG2DBB2JGZWJEQ",
            ),
        ] {
            let id = IdentityId(
                vector(TRANSPORT, &format!("{role} CR2SE ID"))
                    .try_into()
                    .unwrap(),
            );
            assert_eq!(format(id), text);
        }
        assert_eq!(
            format(IdentityId([0; 32])),
            format!("cr2se:{}", "A".repeat(52))
        );
        assert_eq!(
            format(IdentityId([255; 32])),
            format!("cr2se:{}Q", "7".repeat(51))
        );
        for position in 0..256 {
            let mut id = [0; 32];
            id[position / 8] = 1 << (7 - position % 8);
            let text = format(IdentityId(id));
            assert_eq!(text.len(), 58);
            assert!(!text.contains('='));
            assert!(matches!(text.as_bytes()[57], b'A' | b'Q'));
        }
    }
    // Case is the only accepted spelling variation; ambiguous inputs and nonzero trailing bits fail.
    #[test]
    fn parser_rejects_ambiguity_without_trimming() {
        let canonical = "cr2se:3FEKTKCHTGJQLPTATWZZ3HFFBN6JGGCOYNV56CAPN5DECXQZSJOQ";
        assert_eq!(parse(canonical), parse(&canonical.to_ascii_lowercase()));
        assert_eq!(
            normalize(&canonical.to_ascii_lowercase()).unwrap(),
            canonical
        );
        for bad in [
            canonical[6..].to_string(),
            canonical.replacen("cr2se:", "CR2SE:", 1),
            canonical[..57].to_string(),
            format!("{canonical}A"),
            format!("{canonical}="),
            format!(" {canonical}"),
            format!("{canonical}\n"),
            canonical.replacen('F', " ", 1),
            canonical.replacen('F', "0", 1),
            canonical.replacen('F', "Ｆ", 1),
            canonical.replacen('F', "!", 1),
        ] {
            assert!(parse(&bad).is_err(), "{bad}");
        }
        for symbol in ALPHABET {
            let mut value = canonical[..57].to_string();
            value.push(char::from(*symbol));
            assert_eq!(parse(&value).is_ok(), matches!(*symbol, b'A' | b'Q'));
        }
        for position in 0..256 {
            let mut bytes = [0; 32];
            bytes[position / 8] = 1 << (position % 8);
            let id = IdentityId(bytes);
            assert_eq!(parse(&format(id)).unwrap(), id);
        }
        assert!(normalize(" cr2se:").is_err());
    }
}
