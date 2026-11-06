//! Bounded local Board document validation and public durable snapshot inspection.
use crate::modules::{
    board::{Error, config::Limits, offering::Board},
    identity::api::IdentityId,
};
use std::{io::Read, path::Path};

// Read one bounded regular public file; never load the whole untrusted path before checking its budget.
fn read(path: &Path, maximum: usize) -> Result<Vec<u8>, Error> {
    tracing::trace!("Reading a bounded local Board document");
    let file = std::fs::File::open(path).map_err(|_| Error::Storage)?;
    let m = file.metadata().map_err(|_| Error::Storage)?;
    if !m.is_file() || m.len() > maximum as u64 {
        return Err(Error::Limit);
    }
    let mut bytes = Vec::new();
    file.take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Storage)?;
    if bytes.len() > maximum {
        return Err(Error::Limit);
    }
    Ok(bytes)
}
// Validate syntax/structure without pretending that absent service handlers can publish these offerings.
pub(super) fn validate(path: &Path) -> Result<String, String> {
    tracing::trace!("Validating a local Board file without runtime publication");
    let result = (|| {
        let l = Limits::default();
        let bytes = read(path, l.document_bytes)?;
        let board = Board::parse(&bytes, l)?;
        Ok::<_, Error>(format!(
            "Board format valid: {} offerings; definitions not checked; local publishability not established.\n",
            board.offerings.len()
        ))
    })();
    result.map_err(|e| format!("Board validation failed ({e:?})"))
}
// Inspect only an existing protected Linux store; corruption never creates a replacement publication.
#[cfg(target_os = "linux")]
pub(super) fn show(path: &Path, publisher: IdentityId) -> Result<String, String> {
    use crate::modules::board::{
        definition::Definition, linux::FileStore, persistence::Persistence,
    };
    tracing::trace!("Inspecting public Board state from an existing protected store");
    let result = (|| {
        if !path.is_dir() {
            return Err(Error::Missing);
        }
        let l = Limits::default();
        let mut store = FileStore::open(path, l)?;
        let record = store.load()?.ok_or(Error::Missing)?;
        if record.publisher != publisher {
            return Err(Error::Denied);
        }
        let board = Board::parse(&record.board, l)?;
        if record.definitions.len() != board.offerings.len() {
            return Err(Error::Invalid);
        }
        for (id, o) in &board.offerings {
            Definition::parse(record.definitions.get(id).ok_or(Error::Missing)?, o, l)?;
        }
        String::from_utf8(record.board).map_err(|_| Error::Invalid)
    })();
    result
        .map(|s| format!("{s}\n"))
        .map_err(|e| format!("Board inspection failed ({e:?})"))
}
// The protected persistence adapter is currently Linux-only and fails explicitly elsewhere.
#[cfg(not(target_os = "linux"))]
pub(super) fn show(_path: &Path, _publisher: IdentityId) -> Result<String, String> {
    tracing::trace!("Refusing unsupported protected Board inspection");
    Err("Board inspection is supported only on Linux".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    // Missing inputs fail without creating files; exact bounded reads reject excessive input.
    #[test]
    fn missing_inputs_have_no_creation_effect() {
        let p =
            std::env::temp_dir().join(format!("talksphere-absent-board-{}", std::process::id()));
        assert!(validate(&p).is_err());
        assert!(show(&p, IdentityId([1; 32])).is_err());
        assert!(!p.exists());
    }
}
