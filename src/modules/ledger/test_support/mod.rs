//! Test-owned deterministic Ledger dependencies; never linked into production builds.
use std::collections::VecDeque;

/// Failure timing relative to an atomic durable commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cut {
    /// No durable effect occurred.
    Before,
    /// Durable effect occurred but its acknowledgement was lost.
    After,
    /// Durable effect and acknowledgement both succeeded.
    None,
}
/// Minimal independent durable register for exercising future transaction ports.
pub struct Store {
    durable: Vec<u8>,
    cuts: VecDeque<Cut>,
}
impl Store {
    // Supply the complete script; exhaustion fails rather than silently fabricating success.
    pub fn new(initial: &[u8], cuts: impl IntoIterator<Item = Cut>) -> Self {
        Self {
            durable: initial.to_vec(),
            cuts: cuts.into_iter().collect(),
        }
    }
    // Emulate one atomic commit, distinguishing known failure from possible commitment.
    pub fn commit(&mut self, bytes: &[u8]) -> Result<(), super::Error> {
        let cut = self
            .cuts
            .pop_front()
            .expect("Ledger persistence script exhausted");
        if cut == Cut::Before {
            return Err(super::Error::Storage);
        }
        self.durable = bytes.to_vec();
        if cut == Cut::After {
            return Err(super::Error::Unknown);
        }
        Ok(())
    }
    // Restart receives only a copied durable image, never a previous volatile owner.
    pub fn reopen(&self) -> Vec<u8> {
        self.durable.clone()
    }
}

// A known precommit failure preserves old state; lost acknowledgement preserves the new state.
#[test]
fn durable_cuts_are_distinct() {
    let mut s = Store::new(b"old", [Cut::Before, Cut::After, Cut::None]);
    assert_eq!(s.commit(b"bad"), Err(super::Error::Storage));
    assert_eq!(s.reopen(), b"old");
    assert_eq!(s.commit(b"new"), Err(super::Error::Unknown));
    assert_eq!(s.reopen(), b"new");
    assert_eq!(s.commit(b"final"), Ok(()));
    assert_eq!(s.reopen(), b"final");
}
// Unexpected extra writes are harness failures instead of unplanned successful operations.
#[test]
#[should_panic(expected = "Ledger persistence script exhausted")]
fn exhausted_script_fails() {
    let mut s = Store::new(b"", []);
    let _ = s.commit(b"extra");
}
