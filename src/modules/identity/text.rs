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
}
