//! Deterministic test-only dependencies; none may be selected by production composition.
use super::{
    api::{Contact, IdentityId},
    error::IdentityError,
    ports::{ContactStore, Cryptography, KeyStore},
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
};

pub use crate::modules::cryptography::test_support::{ScriptedEntropy, hex};

pub struct CryptoCall {
    pub operation: &'static str,
    pub inputs: Vec<Vec<u8>>,
    pub result: Result<Vec<u8>, IdentityError>,
}
pub struct ScriptedCrypto(pub RefCell<VecDeque<CryptoCall>>);
impl ScriptedCrypto {
    // Consume one expected call and reject unexpected order, bytes or script exhaustion.
    fn call(
        &self,
        operation: &'static str,
        inputs: Vec<Vec<u8>>,
    ) -> Result<Vec<u8>, IdentityError> {
        let call = self
            .0
            .borrow_mut()
            .pop_front()
            .ok_or(IdentityError::Unavailable)?;
        assert_eq!((call.operation, call.inputs), (operation, inputs));
        call.result
    }
}
impl Cryptography for ScriptedCrypto {
    // Return a scripted exact digest without computing the expected production result.
    fn sha256(&self, input: &[u8]) -> Result<[u8; 32], IdentityError> {
        self.call("hash", vec![input.to_vec()])?
            .try_into()
            .map_err(|_| IdentityError::InvalidInput)
    }
    // Record validation before providing the requested success or failure.
    fn validate_signing_key(&self, input: &[u8]) -> Result<(), IdentityError> {
        self.call("key", vec![input.to_vec()]).map(|_| ())
    }
    // Verify operation ordering and exact public/message/signature inputs.
    fn verify(&self, key: &[u8], message: &[u8], signature: &[u8]) -> Result<(), IdentityError> {
        self.call(
            "verify",
            vec![key.to_vec(), message.to_vec(), signature.to_vec()],
        )
        .map(|_| ())
    }
}

#[derive(Default)]
pub struct MemoryStore {
    pub durable: Arc<Mutex<Option<zeroize::Zeroizing<Vec<u8>>>>>,
    pub faults: VecDeque<IdentityError>,
}
impl MemoryStore {
    // Simulate a new process retaining only committed state, not transient failures.
    pub fn restart(&self) -> Self {
        Self {
            durable: self.durable.clone(),
            faults: VecDeque::new(),
        }
    }
}
impl KeyStore for MemoryStore {
    type Administration = bool;
    type Record = zeroize::Zeroizing<Vec<u8>>;
    // Refuse unauthorized/faulted installation without exposing partial persistent state.
    fn install(&mut self, authority: &bool, record: Self::Record) -> Result<(), IdentityError> {
        if !authority {
            return Err(IdentityError::AccessDenied);
        }
        if let Some(error) = self.faults.pop_front() {
            return Err(error);
        }
        let mut state = self.durable.lock().unwrap();
        if state.is_some() {
            return Err(IdentityError::Conflict);
        }
        *state = Some(record);
        Ok(())
    }
    // Return an isolated protected copy only for an authorized existing record.
    fn open(&self, authority: &bool) -> Result<Self::Record, IdentityError> {
        if !authority {
            return Err(IdentityError::AccessDenied);
        }
        self.durable
            .lock()
            .unwrap()
            .clone()
            .ok_or(IdentityError::Unavailable)
    }
}
#[derive(Default)]
pub struct MemoryContacts(pub BTreeMap<IdentityId, Contact>);
impl ContactStore for MemoryContacts {
    type Authority = bool;
    // Read a copied hint without granting any endpoint authentication.
    fn get(&self, authority: &bool, id: IdentityId) -> Result<Option<Contact>, IdentityError> {
        if !authority {
            return Err(IdentityError::AccessDenied);
        }
        Ok(self.0.get(&id).copied())
    }
    // Replace a test contact only after its local authority check.
    fn put(&mut self, authority: &bool, contact: Contact) -> Result<(), IdentityError> {
        if !authority {
            return Err(IdentityError::AccessDenied);
        }
        self.0.insert(contact.expected_id, contact);
        Ok(())
    }
}

// Decode one exact label from a pinned upstream vector, without computing expected bytes.
pub fn vector(source: &str, label: &str) -> Vec<u8> {
    let tail = source
        .split(&format!("\n{label}:\n"))
        .nth(1)
        .expect("literal fixture label");
    hex(&tail
        .lines()
        .take_while(|line| !line.is_empty() && line.chars().all(|c| c.is_ascii_hexdigit()))
        .collect::<String>())
}
pub const TRANSPORT: &str = include_str!("fixtures/transport.md");
pub const ENVELOPE: &str = include_str!("fixtures/cryptography.md");
pub const NEGATIVE: &str = include_str!("fixtures/negative-signatures.md");

// Scripted crypto observes exact inputs; exhaustion and wrong-width results cannot pass.
#[test]
fn crypto_scripts_are_strict() {
    let crypto = ScriptedCrypto(RefCell::new(VecDeque::from([
        CryptoCall {
            operation: "hash",
            inputs: vec![vec![1]],
            result: Ok(vec![2; 32]),
        },
        CryptoCall {
            operation: "key",
            inputs: vec![vec![3]],
            result: Err(IdentityError::InvalidInput),
        },
        CryptoCall {
            operation: "verify",
            inputs: vec![vec![1], vec![2], vec![3]],
            result: Ok(vec![]),
        },
        CryptoCall {
            operation: "hash",
            inputs: vec![vec![]],
            result: Ok(vec![]),
        },
    ])));
    assert_eq!(crypto.sha256(&[1]), Ok([2; 32]));
    assert_eq!(
        crypto.validate_signing_key(&[3]),
        Err(IdentityError::InvalidInput)
    );
    assert_eq!(crypto.verify(&[1], &[2], &[3]), Ok(()));
    assert_eq!(crypto.sha256(&[]), Err(IdentityError::InvalidInput));
    assert_eq!(crypto.sha256(&[]), Err(IdentityError::Unavailable));
}
// Store restart preserves only committed records and denied/faulted calls install nothing.
#[test]
fn durable_substitute_requires_complete_authorized_install() {
    let mut store = MemoryStore::default();
    assert_eq!(store.open(&false), Err(IdentityError::AccessDenied));
    assert_eq!(
        store.install(&false, vec![1].into()),
        Err(IdentityError::AccessDenied)
    );
    store.faults.push_back(IdentityError::Unavailable);
    assert_eq!(
        store.install(&true, vec![1].into()),
        Err(IdentityError::Unavailable)
    );
    assert_eq!(store.restart().open(&true), Err(IdentityError::Unavailable));
    store.install(&true, vec![2].into()).unwrap();
    assert_eq!(&**store.restart().open(&true).unwrap(), &[2]);
    assert_eq!(
        store.install(&true, vec![3].into()),
        Err(IdentityError::Conflict)
    );
}
// Contact reads are immutable snapshots and entropy fixtures fail when exhausted.
#[test]
fn hints_entropy_and_vectors_are_independent() {
    use crate::modules::cryptography::ports::Entropy;
    let mut contacts = MemoryContacts::default();
    let contact = Contact {
        expected_id: IdentityId([1; 32]),
        endpoint: None,
    };
    assert_eq!(
        contacts.get(&false, contact.expected_id),
        Err(IdentityError::AccessDenied)
    );
    assert_eq!(
        contacts.put(&false, contact),
        Err(IdentityError::AccessDenied)
    );
    contacts.put(&true, contact).unwrap();
    let copy = contacts.get(&true, contact.expected_id).unwrap().unwrap();
    contacts
        .put(
            &true,
            Contact {
                endpoint: Some("127.0.0.1:1".parse().unwrap()),
                ..contact
            },
        )
        .unwrap();
    assert_eq!(copy.endpoint, None);
    let mut entropy = ScriptedEntropy(VecDeque::from([Ok(vec![7])]));
    let mut byte = [0];
    entropy.fill(&mut byte).unwrap();
    assert_eq!(byte, [7]);
    assert!(entropy.fill(&mut byte).is_err());
    assert_eq!(vector(TRANSPORT, "initiator Ed25519 seed").len(), 32);
    assert_eq!(
        vector(ENVELOPE, "encryption-key binding signature").len(),
        64
    );
    assert!(NEGATIVE.contains("IbadSign"));
}
