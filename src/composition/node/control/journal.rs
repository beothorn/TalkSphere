//! Bounded durable record of recent control operation outcomes.
//!
//! Callers: the command handler. It lets an operator see which paid operations ended with an
//! unknown outcome after a lost reply or restart; inspecting it never retries anything. The
//! file is replaced atomically and holds no object content, keys or peer-supplied text.
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::Mutex,
};

/// Most records retained; older ones are dropped first.
pub const CAPACITY: usize = 128;

/// How an operation ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Completed and validated.
    Ok,
    /// Refused or failed with no remote commitment.
    Failed,
    /// The remote effect may have committed; the operation was not retried.
    Unknown,
}
impl Status {
    // Stable stored and displayed spelling.
    fn name(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }
}

/// One retained outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// Request identifier chosen by the caller.
    pub id: u64,
    /// Operation name.
    pub op: String,
    /// Lease identifier in hex, when the operation concerns one.
    pub lease: Option<String>,
    /// How it ended.
    pub status: Status,
    /// Unix seconds when it ended.
    pub at: u64,
}

/// File-backed journal.
pub struct Journal {
    path: PathBuf,
    records: Mutex<VecDeque<Record>>,
}
impl Journal {
    /// Open the journal, starting empty when the file is absent.
    /// # Errors
    /// A present file that is malformed is refused rather than silently discarded.
    pub fn open(path: &Path) -> Result<Self, String> {
        tracing::trace!("Opening the operations journal");
        let records = match std::fs::read(path) {
            Ok(bytes) => parse(&bytes)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => VecDeque::new(),
            Err(_) => return Err("operations journal is unreadable".into()),
        };
        Ok(Self {
            path: path.to_path_buf(),
            records: Mutex::new(records),
        })
    }

    /// Append one outcome and persist; a persistence failure is logged and does not fail the operation.
    pub fn record(&self, record: Record) {
        tracing::trace!("Recording a control operation outcome");
        let Ok(mut records) = self.records.lock() else {
            return;
        };
        records.push_back(record);
        while records.len() > CAPACITY {
            records.pop_front();
        }
        if self.persist(&records).is_err() {
            tracing::warn!(
                "The operations journal could not be written; recent outcomes may be lost on restart"
            );
        }
    }

    /// Return all retained records, oldest first.
    pub fn all(&self) -> Vec<Record> {
        tracing::trace!("Reading the operations journal");
        self.records
            .lock()
            .map_or_else(|_| Vec::new(), |r| r.iter().cloned().collect())
    }

    /// Return only records whose outcome is unknown.
    pub fn unknown(&self) -> Vec<Record> {
        tracing::trace!("Reading unknown operation outcomes");
        self.all()
            .into_iter()
            .filter(|r| r.status == Status::Unknown)
            .collect()
    }

    // Replace the file atomically through a private temporary.
    fn persist(&self, records: &VecDeque<Record>) -> Result<(), std::io::Error> {
        tracing::trace!("Persisting the operations journal");
        let document =
            json!({"version": 1, "records": records.iter().map(to_json).collect::<Vec<_>>()});
        let temporary = self.path.with_extension("tmp");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&temporary)?;
        file.write_all(&serde_json::to_vec(&document)?)?;
        file.sync_all()?;
        std::fs::rename(&temporary, &self.path)
    }
}

/// JSON form of one record, shared by the file and control replies.
pub fn to_json(record: &Record) -> Value {
    tracing::trace!("Encoding an operation record");
    json!({"id": record.id, "op": record.op, "lease": record.lease, "status": record.status.name(), "at": record.at})
}

// Parse a stored journal; any malformed member refuses the whole file.
fn parse(bytes: &[u8]) -> Result<VecDeque<Record>, String> {
    tracing::trace!("Parsing the operations journal");
    let bad = || "operations journal is malformed".to_string();
    let value: Value = serde_json::from_slice(bytes).map_err(|_| bad())?;
    if value["version"].as_u64() != Some(1) {
        return Err(bad());
    }
    let mut records = VecDeque::new();
    for item in value["records"].as_array().ok_or_else(bad)? {
        let status = match item["status"].as_str().ok_or_else(bad)? {
            "ok" => Status::Ok,
            "failed" => Status::Failed,
            "unknown" => Status::Unknown,
            _ => return Err(bad()),
        };
        records.push_back(Record {
            id: item["id"].as_u64().ok_or_else(bad)?,
            op: item["op"].as_str().ok_or_else(bad)?.to_string(),
            lease: item["lease"].as_str().map(str::to_string),
            status,
            at: item["at"].as_u64().ok_or_else(bad)?,
        });
    }
    if records.len() > CAPACITY {
        return Err(bad());
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn path() -> PathBuf {
        let mut nonce = [0; 8];
        getrandom::fill(&mut nonce).unwrap();
        std::env::temp_dir().join(format!("journal-{}.json", u64::from_le_bytes(nonce)))
    }
    fn record(id: u64, status: Status) -> Record {
        Record {
            id,
            op: "store".into(),
            lease: Some("ab".into()),
            status,
            at: 5,
        }
    }
    // Records persist across reopen and unknown outcomes can be listed separately.
    #[test]
    fn records_persist_and_unknown_are_filterable() {
        let path = path();
        let journal = Journal::open(&path).unwrap();
        journal.record(record(1, Status::Ok));
        journal.record(record(2, Status::Unknown));
        journal.record(record(3, Status::Failed));
        let reopened = Journal::open(&path).unwrap();
        assert_eq!(reopened.all().len(), 3);
        assert_eq!(reopened.unknown(), vec![record(2, Status::Unknown)]);
        assert_eq!(to_json(&record(9, Status::Ok))["status"], "ok");
    }
    // The journal keeps only the most recent records.
    #[test]
    fn capacity_is_bounded() {
        let journal = Journal::open(&path()).unwrap();
        for id in 0..(CAPACITY as u64 + 10) {
            journal.record(record(id, Status::Ok));
        }
        let all = journal.all();
        assert_eq!(all.len(), CAPACITY);
        assert_eq!(all[0].id, 10);
    }
    // A malformed file is refused rather than discarded; a write failure does not fail recording.
    #[test]
    fn malformed_files_are_refused_and_write_failures_tolerated() {
        let path = path();
        for bad in [
            "junk",
            r#"{"version":2,"records":[]}"#,
            r#"{"version":1,"records":[{"id":1}]}"#,
        ] {
            std::fs::write(&path, bad).unwrap();
            assert!(Journal::open(&path).is_err());
        }
        let journal =
            Journal::open(&std::env::temp_dir().join("missing-dir-xyz/ops.json")).unwrap();
        journal.record(record(1, Status::Ok));
        assert_eq!(journal.all().len(), 1);
    }
}
