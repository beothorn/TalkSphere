//! Media types of page resources: the exact form `type/subtype` of Page version one.
//!
//! Callers: the page manifest, the hosted page assembly and the publisher-served page. The type labels the content and never transforms
//! it: there is no negotiation, compression, character-set conversion or decompression, and a viewer that does not trust a label may treat
//! the bytes as `application/octet-stream`.
use super::error::Error;

/// Largest number of characters in the type or the subtype.
pub const MAXIMUM_PART_CHARACTERS: usize = 63;

/// A validated media type: one slash, no whitespace or parameters, and a type and subtype of 1 to 63 lowercase ASCII characters from
/// `a-z 0-9 ! # $ & ^ _ . + -`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MediaType(String);

impl MediaType {
    /// The label of the required root resource.
    pub const HTML: &'static str = "text/html";

    /// Validate a media type.
    /// # Errors
    /// `Invalid` for any text that is not exactly `type/subtype` in the allowed alphabet and lengths (uppercase, whitespace, parameters,
    /// a missing or extra slash, an empty or 64-character part, non-ASCII).
    pub fn parse(text: &str) -> Result<Self, Error> {
        tracing::trace!("Validating a media type");
        let (kind, subtype) = text.split_once('/').ok_or(Error::Invalid)?;
        if valid_part(kind) && valid_part(subtype) {
            Ok(Self(text.to_string()))
        } else {
            Err(Error::Invalid)
        }
    }

    /// The media type as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether this is exactly `text/html`, the type the root resource must have.
    pub fn is_html(&self) -> bool {
        self.0 == Self::HTML
    }
}

// One part: 1..=63 characters, each a lowercase letter, a digit or one of the allowed punctuation marks. A second slash is not in the
// alphabet, so it makes the subtype invalid.
fn valid_part(part: &str) -> bool {
    (1..=MAXIMUM_PART_CHARACTERS).contains(&part.len())
        && part.bytes().all(|b| {
            b.is_ascii_lowercase()
                || b.is_ascii_digit()
                || matches!(
                    b,
                    b'!' | b'#' | b'$' | b'&' | b'^' | b'_' | b'.' | b'+' | b'-'
                )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    // PG-04: valid media types keep their exact label, including the boundary lengths.
    #[test]
    fn valid_media_types_are_retained() {
        for text in [
            "text/html",
            "text/css",
            "application/javascript",
            "image/svg+xml",
            "application/octet-stream",
            "a/b",
            "x-y_z.1/#$&^!+-",
        ] {
            assert_eq!(MediaType::parse(text).unwrap().as_str(), text);
        }
        let longest = format!(
            "{}/{}",
            "a".repeat(MAXIMUM_PART_CHARACTERS),
            "b".repeat(MAXIMUM_PART_CHARACTERS)
        );
        assert!(MediaType::parse(&longest).is_ok());
        assert!(MediaType::parse("text/html").unwrap().is_html());
        assert!(!MediaType::parse("text/plain").unwrap().is_html());
    }

    // PG-04: uppercase, parameters, whitespace, a missing or extra slash, an empty part, a 64-character part and non-ASCII are invalid.
    #[test]
    fn invalid_media_types_are_rejected() {
        let long = "a".repeat(MAXIMUM_PART_CHARACTERS + 1);
        for text in [
            "",
            "Text/html",
            "text/HTML",
            "text/html; charset=utf-8",
            "text /html",
            "text/ html",
            " text/html",
            "text",
            "a/b/c",
            "/html",
            "text/",
            "text/h\u{e9}",
            "t\u{e9}xt/html",
            "text/ht\tml",
            "text/ht,ml",
            "text/ht=ml",
            "text/ht*ml",
        ] {
            assert_eq!(MediaType::parse(text), Err(Error::Invalid), "{text:?}");
        }
        assert_eq!(MediaType::parse(&format!("{long}/b")), Err(Error::Invalid));
        assert_eq!(MediaType::parse(&format!("a/{long}")), Err(Error::Invalid));
    }
}
