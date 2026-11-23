//! Canonical page paths: protocol strings naming one resource within a page.
//!
//! Callers: the page manifest, the hosted page assembly and the publisher-served page. A path is a UTF-8 protocol string, never a
//! filesystem path or URI: percent signs, question marks and number signs are ordinary characters and nothing is decoded or normalized.
//! Paths are case- and normalization-sensitive, so `/About` and `/about` are different names. Validation happens completely before a path
//! is used for any lookup.
use super::error::Error;

/// Largest number of bytes in one path component.
pub const MAXIMUM_COMPONENT_BYTES: usize = 255;

/// A validated canonical path: the root `/`, or a slash followed by one or more valid components separated by single slashes with no
/// trailing slash.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalPath(String);

impl CanonicalPath {
    /// The root path `/`.
    pub fn root() -> Self {
        tracing::trace!("Creating the root path");
        Self("/".to_string())
    }

    /// Validate path bytes: they must be one valid UTF-8 string that is a canonical path.
    /// # Errors
    /// `Invalid` for invalid UTF-8 (including surrogates and overlong forms), a missing initial slash, a trailing slash, an empty, dot or
    /// parent component, a component of 0 or more than 255 bytes, a control character U+0000..U+001F or U+007F, or a reverse solidus.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        tracing::trace!("Validating a canonical path");
        let text = std::str::from_utf8(bytes).map_err(|_| Error::Invalid)?;
        if text == "/" {
            return Ok(Self::root());
        }
        let rest = text.strip_prefix('/').ok_or(Error::Invalid)?;
        if rest.split('/').all(valid_component) {
            Ok(Self(text.to_string()))
        } else {
            Err(Error::Invalid)
        }
    }

    /// The path as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The path's UTF-8 bytes, whose count is the path size.
    pub fn bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Whether this is the root path.
    pub fn is_root(&self) -> bool {
        self.0 == "/"
    }

    /// The path size in bytes, to compare with an offering's `maximumPathBytes`.
    pub fn size(&self) -> usize {
        self.0.len()
    }
}

// One component is 1..=255 bytes, is not "." or "..", and has no control character, slash or reverse solidus.
fn valid_component(component: &str) -> bool {
    (1..=MAXIMUM_COMPONENT_BYTES).contains(&component.len())
        && component != "."
        && component != ".."
        && !component
            .chars()
            .any(|c| c <= '\u{1f}' || c == '\u{7f}' || c == '/' || c == '\\')
}

#[cfg(test)]
mod tests {
    use super::*;

    // PG-03: the valid paths are literal protocol paths, including percent signs, query-like and fragment-like text.
    #[test]
    fn valid_paths_are_literal() {
        for text in [
            "/",
            "/about",
            "/a/b",
            "/%2e%2e",
            "/a?b#c",
            "/posts/first-post.html",
            "/Ünïcode/é",
        ] {
            let path = CanonicalPath::parse(text.as_bytes()).unwrap();
            assert_eq!((path.as_str(), path.size()), (text, text.len()));
        }
        assert!(CanonicalPath::parse(b"/").unwrap().is_root());
        assert!(!CanonicalPath::parse(b"/a").unwrap().is_root());
        assert_eq!(CanonicalPath::root(), CanonicalPath::parse(b"/").unwrap());
        let boundary = format!("/{}", "a".repeat(MAXIMUM_COMPONENT_BYTES));
        assert!(CanonicalPath::parse(boundary.as_bytes()).is_ok());
    }

    // PG-03: every invalid shape is rejected before any use: no slash, trailing slash, empty, dot and parent components, backslash,
    // control characters (including DEL and NUL), invalid UTF-8, a surrogate, an overlong encoding and a 256-byte component.
    #[test]
    fn invalid_paths_are_rejected() {
        let too_long = format!("/{}", "a".repeat(MAXIMUM_COMPONENT_BYTES + 1));
        let cases: Vec<Vec<u8>> = vec![
            b"".to_vec(),
            b"about".to_vec(),
            b"/about/".to_vec(),
            b"//a".to_vec(),
            b"/a//b".to_vec(),
            b"/a/./b".to_vec(),
            b"/a/../b".to_vec(),
            b"/.".to_vec(),
            b"/..".to_vec(),
            b"/images\\photo.jpg".to_vec(),
            b"/a\x01b".to_vec(),
            b"/a\x00b".to_vec(),
            b"/a\x7fb".to_vec(),
            b"/a\nb".to_vec(),
            vec![b'/', 0xff, 0xfe],
            vec![b'/', 0xed, 0xa0, 0x80],
            vec![b'/', 0xc0, 0xaf],
            too_long.into_bytes(),
        ];
        for bytes in cases {
            assert_eq!(
                CanonicalPath::parse(&bytes),
                Err(Error::Invalid),
                "{bytes:?}"
            );
        }
    }

    // PG-03: paths differing in case or Unicode normalization are different names, and ordering is by bytes.
    #[test]
    fn names_are_case_and_normalization_sensitive() {
        let (upper, lower) = (
            CanonicalPath::parse(b"/About").unwrap(),
            CanonicalPath::parse(b"/about").unwrap(),
        );
        assert_ne!(upper, lower);
        assert!(upper < lower);
        let composed = CanonicalPath::parse("/\u{e9}".as_bytes()).unwrap();
        let decomposed = CanonicalPath::parse("/e\u{301}".as_bytes()).unwrap();
        assert_ne!(composed, decomposed);
        assert_eq!(composed.bytes(), "/\u{e9}".as_bytes());
    }
}
