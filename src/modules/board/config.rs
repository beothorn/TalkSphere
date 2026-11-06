//! Finite local resource policy checked before Board decoding or platform effects.
use super::Error;

/// Positive local budgets, separate from fixed peer protocol maxima.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Maximum input bytes for one document, including unknown metadata.
    pub document_bytes: usize,
    /// Maximum nested JSON containers in a standalone document.
    pub depth: usize,
    /// Maximum JSON values and object members counted during parsing.
    pub nodes: usize,
    /// Maximum UTF-8 bytes in one decoded string.
    pub string_bytes: usize,
    /// Maximum elements in one array.
    pub array_elements: usize,
    /// Maximum definitions in one candidate publication.
    pub definitions: usize,
    /// Maximum aggregate Board and definition bytes per publication.
    pub publication_bytes: usize,
    /// Maximum remote publisher entries retained by a cache.
    pub cache_entries: usize,
    /// Maximum live snapshot revisions owned by the Board.
    pub snapshots: usize,
    /// Maximum cache lifetime in whole monotonic seconds.
    pub cache_seconds: u64,
}
impl Default for Limits {
    /// Choose bounded defaults; response wrappers impose additional size/depth checks later.
    fn default() -> Self {
        tracing::trace!("Choosing finite Board resource defaults");
        Self {
            document_bytes: 1_048_576,
            depth: 31,
            nodes: 65_536,
            string_bytes: 1_048_576,
            array_elements: 4096,
            definitions: 1024,
            publication_bytes: 16_777_216,
            cache_entries: 64,
            snapshots: 128,
            cache_seconds: 300,
        }
    }
}
impl Limits {
    /// Validate policy before allocation; lower local limits may refuse otherwise valid documents.
    /// # Errors
    /// Returns Invalid for zero budgets or inconsistent aggregate/string/depth bounds.
    pub fn validate(&self) -> Result<(), Error> {
        tracing::trace!("Validating Board resource budgets before effects");
        let values = [
            self.document_bytes,
            self.depth,
            self.nodes,
            self.string_bytes,
            self.array_elements,
            self.definitions,
            self.publication_bytes,
            self.cache_entries,
            self.snapshots,
        ];
        if values.contains(&0)
            || self.cache_seconds == 0
            || self.depth > 31
            || self.document_bytes > 1_048_576
            || self.string_bytes > self.document_bytes
            || self.publication_bytes < self.document_bytes
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    // Every independently configurable budget must reject zero before parsing or persistence.
    #[test]
    fn zero_budgets_are_rejected() {
        let good = Limits::default();
        assert_eq!(good.validate(), Ok(()));
        for i in 0..10 {
            let mut bad = good;
            match i {
                0 => bad.document_bytes = 0,
                1 => bad.depth = 0,
                2 => bad.nodes = 0,
                3 => bad.string_bytes = 0,
                4 => bad.array_elements = 0,
                5 => bad.definitions = 0,
                6 => bad.publication_bytes = 0,
                7 => bad.cache_entries = 0,
                8 => bad.snapshots = 0,
                _ => bad.cache_seconds = 0,
            }
            assert_eq!(bad.validate(), Err(Error::Invalid));
        }
    }
    // Wrapper depth and aggregate/string relationships cannot be configured inconsistently.
    #[test]
    fn inconsistent_limits_are_rejected() {
        for bad in [
            Limits {
                depth: 32,
                ..Limits::default()
            },
            Limits {
                document_bytes: 1_048_577,
                ..Limits::default()
            },
            Limits {
                string_bytes: 1_048_577,
                ..Limits::default()
            },
            Limits {
                publication_bytes: 1,
                ..Limits::default()
            },
        ] {
            assert_eq!(bad.validate(), Err(Error::Invalid));
        }
    }
}
