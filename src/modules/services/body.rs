//! Ordered bounded complete-body ownership; provisional fragments never become successful results.
use super::{Error, handler::Blob};
use sha2::{Digest, Sha256};
/// One finite declared body with optional contract-required content hash.
pub struct Body {
    length: usize,
    hash: Option<[u8; 32]>,
    bytes: Vec<u8>,
    failed: bool,
}
impl Body {
    /// Admit declared length before any proportional allocation; reserve no body storage up front.
    /// # Errors
    /// Limit rejects lengths exceeding address space or configured capacity.
    pub fn new(length: u64, hash: Option<[u8; 32]>, maximum: usize) -> Result<Self, Error> {
        tracing::trace!("Admitting finite declared service body");
        let length = usize::try_from(length).map_err(|_| Error::Limit)?;
        if length > maximum {
            return Err(Error::Limit);
        }
        Ok(Self {
            length,
            hash,
            bytes: Vec::new(),
            failed: false,
        })
    }
    /// Append only the next exact byte offset within the declared length; errors poison this body.
    /// Fragment entry logging is intentionally omitted to avoid peer-controlled per-byte log volume.
    /// # Errors
    /// Invalid rejects gaps, overlaps, trailing bytes and any use after a previous malformed fragment.
    pub fn push(&mut self, offset: u64, bytes: &[u8]) -> Result<(), Error> {
        if self.failed {
            return Err(Error::Invalid);
        }
        if usize::try_from(offset).ok() != Some(self.bytes.len())
            || self
                .bytes
                .len()
                .checked_add(bytes.len())
                .is_none_or(|n| n > self.length)
        {
            self.failed = true;
            return Err(Error::Invalid);
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    /// Consume only exact complete input and verify its agreed content hash before returning retained bytes.
    /// # Errors
    /// Invalid rejects missing bytes, prior failure and content mismatch; no provisional Blob is returned.
    pub fn finish(self) -> Result<Blob, Error> {
        tracing::trace!("Validating complete service body before publication");
        if self.failed || self.bytes.len() != self.length {
            return Err(Error::Invalid);
        }
        if self
            .hash
            .is_some_and(|expected| <[u8; 32]>::from(Sha256::digest(&self.bytes)) != expected)
        {
            return Err(Error::Invalid);
        }
        Blob::new(self.bytes, self.length)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    // Literal SHA256(abc) pins content validation independently of the production hash call.
    #[test]
    fn complete_ordered_body() {
        let hash = [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad,
        ];
        let mut body = Body::new(3, Some(hash), 3).unwrap();
        body.push(0, b"a").unwrap();
        body.push(1, b"bc").unwrap();
        assert_eq!(body.finish().unwrap().bytes(), b"abc");
        assert_eq!(
            Body::new(0, None, 0).unwrap().finish().unwrap().bytes(),
            b""
        );
    }
    // Missing, extra, overlapping and corrupted input never yields complete retained content.
    #[test]
    fn invalid_fragments_and_bounds() {
        assert!(matches!(Body::new(u64::MAX, None, 1), Err(Error::Limit)));
        assert!(matches!(
            Body::new(1, None, 1).unwrap().finish(),
            Err(Error::Invalid)
        ));
        let mut body = Body::new(1, None, 1).unwrap();
        assert_eq!(body.push(1, b"x"), Err(Error::Invalid));
        assert_eq!(body.push(0, b"x"), Err(Error::Invalid));
        assert!(matches!(body.finish(), Err(Error::Invalid)));
        let mut body = Body::new(1, None, 1).unwrap();
        body.push(0, b"x").unwrap();
        assert_eq!(body.push(1, b"y"), Err(Error::Invalid));
        assert!(matches!(body.finish(), Err(Error::Invalid)));
        let mut body = Body::new(1, Some([0; 32]), 1).unwrap();
        body.push(0, b"x").unwrap();
        assert!(matches!(body.finish(), Err(Error::Invalid)));
    }
}
