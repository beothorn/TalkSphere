//! Complete chunk writes tolerate short platform writes and preserve failures before publication.
use crate::modules::services::Error;
use std::io::Write;

// Write every byte or return a typed failure; partial success never means a complete upload chunk.
pub(super) fn write_exact(writer: &mut impl Write, bytes: &[u8]) -> Result<(), Error> {
    tracing::trace!("Writing a complete bounded Storage upload chunk");
    writer.write_all(bytes).map_err(|_| Error::Storage)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Short {
        bytes: Vec<u8>,
        fail_after: Option<usize>,
    }
    impl Write for Short {
        // Emit one byte at a time and inject an error after a chosen number of acknowledged bytes.
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.fail_after == Some(self.bytes.len()) {
                return Err(std::io::Error::other("injected write failure"));
            }
            if bytes.is_empty() {
                return Ok(0);
            }
            self.bytes.push(bytes[0]);
            Ok(1)
        }
        // This helper tests complete writes, not durability barriers, which the real file suite covers.
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    // Multiple short writes complete exactly, while a partial write followed by failure stays failed.
    #[test]
    fn short_and_partial_writes() {
        let mut good = Short {
            bytes: vec![],
            fail_after: None,
        };
        assert_eq!(write_exact(&mut good, b"abc"), Ok(()));
        assert_eq!(good.bytes, b"abc");
        let mut bad = Short {
            bytes: vec![],
            fail_after: Some(1),
        };
        assert_eq!(write_exact(&mut bad, b"abc"), Err(Error::Storage));
        assert_eq!(bad.bytes, b"a");
    }
}
