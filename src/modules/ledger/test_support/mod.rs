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

// Reuse pinned public vector seeds; C/U use independent fixed test-only seeds.
pub fn identities() -> [crate::modules::identity::api::IdentityId; 4] {
    use crate::modules::{
        cryptography::{
            primitives::keys::signing_key,
            test_support::{ScriptedEntropy, transport},
        },
        identity::api::IdentityId,
    };
    [
        transport("initiator Ed25519 seed"),
        transport("acceptor Ed25519 seed"),
        vec![3; 32],
        vec![4; 32],
    ]
    .map(|seed| {
        let mut entropy = ScriptedEntropy(VecDeque::from([Ok(seed)]));
        let key = signing_key(&mut entropy).unwrap();
        let mut input = vec![1, 1];
        input.extend_from_slice(&key.public_key);
        use sha2::Digest;
        IdentityId(sha2::Sha256::digest(&input).into())
    })
}

/// Controlled external-receipt response without synthetic authentication claims.
pub struct Receipts<T>(pub VecDeque<Result<T, super::Error>>);
impl<T> Receipts<T> {
    // Missing or refused issuer evidence cannot turn into a successful payment.
    pub fn next(&mut self) -> Result<T, super::Error> {
        self.0.pop_front().unwrap_or(Err(super::Error::Unknown))
    }
}
// Identity fixtures are reproducible/distinct; scripted entropy and evidence failures stay explicit.
#[test]
fn deterministic_sources_and_refusals() {
    use crate::modules::cryptography::{
        error::CryptoError, ports::Entropy, test_support::ScriptedEntropy,
    };
    let ids = identities();
    assert_eq!(ids, identities());
    for i in 0..4 {
        for j in i + 1..4 {
            assert_ne!(ids[i], ids[j]);
        }
    }
    let mut e = ScriptedEntropy(VecDeque::from([
        Ok(vec![9; 16]),
        Err(CryptoError::Unavailable),
    ]));
    let mut out = [0; 16];
    e.fill(&mut out).unwrap();
    assert_eq!(out, [9; 16]);
    assert!(e.fill(&mut out).is_err());
    assert!(e.fill(&mut out).is_err());
    let mut r = Receipts(VecDeque::from([Ok(7), Err(super::Error::Denied)]));
    assert_eq!(r.next(), Ok(7));
    assert_eq!(r.next(), Err(super::Error::Denied));
    assert_eq!(r.next(), Err(super::Error::Unknown));
}
