//! Strict primitive-call scripts, not a cryptographic implementation.

use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

use super::script::Script;
use crate::modules::transport::ports::{
    DependencyError,
    crypto::{Cryptography, EphemeralKey},
};

#[derive(Default)]
struct SecretState {
    issued: BTreeSet<u64>,
    released: Vec<u64>,
}

#[derive(Clone, Default)]
pub struct SecretLog(Arc<Mutex<SecretState>>);

/// Only a unique test label, with no private key bytes or Clone implementation.
pub struct TestSecret {
    label: u64,
    log: SecretLog,
}

impl SecretLog {
    pub fn issue(&self, label: u64) -> TestSecret {
        let new = self.0.lock().unwrap().issued.insert(label);
        assert!(new, "test secret label reused");
        TestSecret {
            label,
            log: self.clone(),
        }
    }

    pub fn released(&self) -> Vec<u64> {
        self.0.lock().unwrap().released.clone()
    }
}

impl TestSecret {
    pub fn label(&self) -> u64 {
        self.label
    }
}

impl Drop for TestSecret {
    fn drop(&mut self) {
        self.log.0.lock().unwrap().released.push(self.label);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CryptoCall {
    RandomNonce,
    EphemeralKey,
    Sha256(Vec<u8>),
    VerifySignature {
        public_key: [u8; 32],
        message: Vec<u8>,
        signature: [u8; 64],
    },
    Agree {
        private_key: u64,
        remote_key: [u8; 32],
    },
    Extract {
        salt: [u8; 32],
        shared: u64,
    },
    ExpandKey {
        key: u64,
        info: Vec<u8>,
    },
    ExpandNoncePrefix {
        key: u64,
        info: Vec<u8>,
    },
    Tag {
        key: u64,
        nonce: [u8; 24],
        authenticated_data: Vec<u8>,
    },
    VerifyTag {
        key: u64,
        nonce: [u8; 24],
        authenticated_data: Vec<u8>,
        tag: [u8; 16],
    },
}

pub enum CryptoReply {
    Bytes32(Result<[u8; 32], DependencyError>),
    EphemeralKey(Result<EphemeralKey<TestSecret>, DependencyError>),
    Secret(Result<TestSecret, DependencyError>),
    Bytes16(Result<[u8; 16], DependencyError>),
    Verified(Result<(), DependencyError>),
}

pub struct ScriptedCryptography(pub Script<CryptoCall, CryptoReply>);

impl Cryptography for ScriptedCryptography {
    type EphemeralSecret = TestSecret;
    type SharedSecret = TestSecret;
    type PseudorandomKey = TestSecret;
    type IntegrityKey = TestSecret;

    fn random_nonce(&mut self) -> Result<[u8; 32], DependencyError> {
        let CryptoReply::Bytes32(result) = self.0.call(CryptoCall::RandomNonce) else {
            panic!("expected nonce reply")
        };
        result
    }

    fn ephemeral_key(&mut self) -> Result<EphemeralKey<TestSecret>, DependencyError> {
        let CryptoReply::EphemeralKey(result) = self.0.call(CryptoCall::EphemeralKey) else {
            panic!("expected ephemeral key reply")
        };
        result
    }

    fn sha256(&self, bytes: &[u8]) -> Result<[u8; 32], DependencyError> {
        let CryptoReply::Bytes32(result) = self.0.call(CryptoCall::Sha256(bytes.to_vec())) else {
            panic!("expected hash reply")
        };
        result
    }

    fn verify_signature(
        &self,
        public_key: &[u8; 32],
        message: &[u8],
        signature: &[u8; 64],
    ) -> Result<(), DependencyError> {
        let CryptoReply::Verified(result) = self.0.call(CryptoCall::VerifySignature {
            public_key: *public_key,
            message: message.to_vec(),
            signature: *signature,
        }) else {
            panic!("expected signature verification reply")
        };
        result
    }

    fn agree(
        &mut self,
        private_key: TestSecret,
        remote_key: &[u8; 32],
    ) -> Result<TestSecret, DependencyError> {
        let CryptoReply::Secret(result) = self.0.call(CryptoCall::Agree {
            private_key: private_key.label(),
            remote_key: *remote_key,
        }) else {
            panic!("expected agreement reply")
        };
        result
    }

    fn extract(
        &mut self,
        salt: &[u8; 32],
        shared: TestSecret,
    ) -> Result<TestSecret, DependencyError> {
        let CryptoReply::Secret(result) = self.0.call(CryptoCall::Extract {
            salt: *salt,
            shared: shared.label(),
        }) else {
            panic!("expected extraction reply")
        };
        result
    }

    fn expand_key(&mut self, key: &TestSecret, info: &[u8]) -> Result<TestSecret, DependencyError> {
        let CryptoReply::Secret(result) = self.0.call(CryptoCall::ExpandKey {
            key: key.label(),
            info: info.to_vec(),
        }) else {
            panic!("expected key expansion reply")
        };
        result
    }

    fn expand_nonce_prefix(
        &self,
        key: &TestSecret,
        info: &[u8],
    ) -> Result<[u8; 16], DependencyError> {
        let CryptoReply::Bytes16(result) = self.0.call(CryptoCall::ExpandNoncePrefix {
            key: key.label(),
            info: info.to_vec(),
        }) else {
            panic!("expected nonce prefix reply")
        };
        result
    }

    fn tag(
        &self,
        key: &TestSecret,
        nonce: &[u8; 24],
        authenticated_data: &[u8],
    ) -> Result<[u8; 16], DependencyError> {
        let CryptoReply::Bytes16(result) = self.0.call(CryptoCall::Tag {
            key: key.label(),
            nonce: *nonce,
            authenticated_data: authenticated_data.to_vec(),
        }) else {
            panic!("expected tag reply")
        };
        result
    }

    fn verify_tag(
        &self,
        key: &TestSecret,
        nonce: &[u8; 24],
        authenticated_data: &[u8],
        tag: &[u8; 16],
    ) -> Result<(), DependencyError> {
        let CryptoReply::Verified(result) = self.0.call(CryptoCall::VerifyTag {
            key: key.label(),
            nonce: *nonce,
            authenticated_data: authenticated_data.to_vec(),
            tag: *tag,
        }) else {
            panic!("expected tag verification reply")
        };
        result
    }
}

#[cfg(test)]
mod tests;
