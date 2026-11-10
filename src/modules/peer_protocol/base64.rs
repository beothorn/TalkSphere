//! Canonical RFC4648 padded Base64 for fixed control identifiers and inline byte values.
use super::Error;
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
/// Encode exact bytes with standard alphabet and required padding.
pub fn encode(bytes: &[u8]) -> String {
    tracing::trace!("Encoding canonical peer byte representation");
    let mut text = String::new();
    for group in bytes.chunks(3) {
        let a = group[0];
        let b = group.get(1).copied().unwrap_or(0);
        let c = group.get(2).copied().unwrap_or(0);
        text.push(char::from(ALPHABET[(a >> 2) as usize]));
        text.push(char::from(ALPHABET[((a & 3) << 4 | b >> 4) as usize]));
        text.push(if group.len() > 1 {
            char::from(ALPHABET[((b & 15) << 2 | c >> 6) as usize])
        } else {
            '='
        });
        text.push(if group.len() > 2 {
            char::from(ALPHABET[(c & 63) as usize])
        } else {
            '='
        });
    }
    text
}
/// Decode only a canonical spelling within the decoded-byte budget.
/// # Errors
/// Invalid rejects whitespace, padding, alphabet or nonzero unused bits; Limit rejects capacity before allocation.
pub fn decode(text: &str, maximum: usize) -> Result<Vec<u8>, Error> {
    tracing::trace!("Decoding bounded canonical peer bytes");
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return Err(Error::Invalid);
    }
    let padding = usize::from(bytes.last() == Some(&b'='))
        + usize::from(bytes.len() > 1 && bytes[bytes.len() - 2] == b'=');
    let length = (bytes.len() / 4)
        .checked_mul(3)
        .and_then(|n| n.checked_sub(padding))
        .ok_or(Error::Invalid)?;
    if length > maximum {
        return Err(Error::Limit);
    }
    let mut out = Vec::with_capacity(length);
    for (index, group) in bytes.as_chunks::<4>().0.iter().enumerate() {
        let a = digit(group[0])?;
        let b = digit(group[1])?;
        let last = index + 1 == bytes.len() / 4;
        let c = if group[2] == b'=' {
            if !last || group[3] != b'=' || b & 15 != 0 {
                return Err(Error::Invalid);
            }
            0
        } else {
            digit(group[2])?
        };
        let d = if group[3] == b'=' {
            if !last || c & 3 != 0 {
                return Err(Error::Invalid);
            }
            0
        } else {
            digit(group[3])?
        };
        out.push(a << 2 | b >> 4);
        if group[2] != b'=' {
            out.push(b << 4 | c >> 2);
        }
        if group[3] != b'=' {
            out.push(c << 6 | d);
        }
    }
    Ok(out)
}
// Reject every nonstandard symbol; no input byte is repaired or normalized.
fn digit(byte: u8) -> Result<u8, Error> {
    match byte {
        b'A'..=b'Z' => Ok(byte - b'A'),
        b'a'..=b'z' => Ok(byte - b'a' + 26),
        b'0'..=b'9' => Ok(byte - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        _ => Err(Error::Invalid),
    }
}
