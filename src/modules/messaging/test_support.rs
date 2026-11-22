//! Deterministic test doubles for Messaging; compiled only for tests and never usable by production code.
//!
//! A controllable clock, a counting entropy source and test identities with real Ed25519 keys derived from
//! a seed byte, each with injectable failures so dependency-failure paths are testable. The placement
//! store double arrives with the store port in step 12.
use super::{
    error::Error,
    ids::Signature,
    ports::{Clock, Entropy, Signer},
};
use crate::modules::{
    cryptography::{
        credential,
        primitives::signature,
        secrets::{Secret, Signing},
    },
    identity::api::IdentityId,
};
use std::sync::Mutex;
use zeroize::Zeroizing;

/// A clock the test sets and advances by hand.
pub struct FixedClock {
    now: Mutex<Option<u64>>,
}
impl FixedClock {
    /// A clock showing `now`.
    pub fn at(now: u64) -> Self {
        Self {
            now: Mutex::new(Some(now)),
        }
    }
    /// Move time forward.
    pub fn advance(&self, seconds: u64) {
        let mut now = self.now.lock().unwrap();
        *now = now.map(|n| n + seconds);
    }
    /// Set the time.
    pub fn set(&self, value: u64) {
        *self.now.lock().unwrap() = Some(value);
    }
    /// Make every later read fail until `set` is called.
    pub fn fail(&self) {
        *self.now.lock().unwrap() = None;
    }
}
impl Clock for FixedClock {
    fn now(&self) -> Result<u64, Error> {
        self.now.lock().unwrap().ok_or(Error::Unavailable)
    }
}

/// Entropy that yields a predictable, never repeating byte sequence, and can be made to fail.
pub struct SequenceEntropy {
    counter: Mutex<Option<u64>>,
}
impl SequenceEntropy {
    /// A source starting at `start`.
    pub fn new(start: u64) -> Self {
        Self {
            counter: Mutex::new(Some(start)),
        }
    }
    /// Make every later request fail.
    pub fn fail(&self) {
        *self.counter.lock().unwrap() = None;
    }
}
impl Entropy for SequenceEntropy {
    // Each request gets the next counter value spread over its bytes, so two identifiers never match.
    fn fill(&self, bytes: &mut [u8]) -> Result<(), Error> {
        let mut guard = self.counter.lock().unwrap();
        let value = guard.as_mut().ok_or(Error::Unavailable)?;
        *value += 1;
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = value.to_be_bytes()[i % 8] ^ (i as u8);
        }
        Ok(())
    }
}

/// A test identity with a real Ed25519 key derived from one seed byte.
pub struct TestIdentity {
    secret: Secret<Signing>,
    public_key: [u8; 32],
    id: IdentityId,
    signing: Mutex<bool>,
}
impl TestIdentity {
    /// The identity for `seed`; different seeds give different identities.
    pub fn new(seed: u8) -> Self {
        let bytes = Zeroizing::new([seed; 32]);
        let public_key = ed25519_dalek::SigningKey::from_bytes(&bytes)
            .verifying_key()
            .to_bytes();
        let id = IdentityId(credential::identity_id(&public_key).unwrap());
        Self {
            secret: Secret::new(bytes),
            public_key,
            id,
            signing: Mutex::new(true),
        }
    }
    /// Make signing fail from now on.
    pub fn break_signing(&self) {
        *self.signing.lock().unwrap() = false;
    }
    /// The identity's public key.
    pub fn key(&self) -> [u8; 32] {
        self.public_key
    }
}
impl Signer for TestIdentity {
    fn identity(&self) -> IdentityId {
        self.id
    }
    fn public_key(&self) -> [u8; 32] {
        self.public_key
    }
    fn sign(&self, message: &[u8]) -> Result<Signature, Error> {
        if !*self.signing.lock().unwrap() {
            return Err(Error::Unavailable);
        }
        signature::sign(&self.secret, message, 1 << 20)
            .map(Signature)
            .map_err(|_| Error::Invalid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The clock shows, advances and sets time, and reports unavailable after a failure until reset.
    #[test]
    fn the_clock_is_controllable_and_can_fail() {
        let clock = FixedClock::at(1000);
        assert_eq!(clock.now(), Ok(1000));
        clock.advance(10);
        assert_eq!(clock.now(), Ok(1010));
        clock.fail();
        assert_eq!(clock.now(), Err(Error::Unavailable));
        clock.advance(5);
        assert_eq!(clock.now(), Err(Error::Unavailable));
        clock.set(7);
        assert_eq!(clock.now(), Ok(7));
    }

    // Entropy is deterministic across instances, never repeats within one, and fails when told to.
    #[test]
    fn entropy_is_deterministic_unique_and_can_fail() {
        let (a, b) = (SequenceEntropy::new(5), SequenceEntropy::new(5));
        let (mut one, mut two, mut again) = ([0u8; 32], [0u8; 32], [0u8; 32]);
        a.fill(&mut one).unwrap();
        b.fill(&mut two).unwrap();
        assert_eq!(one, two);
        a.fill(&mut again).unwrap();
        assert_ne!(one, again);
        a.fail();
        assert_eq!(a.fill(&mut one), Err(Error::Unavailable));
    }

    // Identities are distinct, sign verifiably with real Ed25519 and can lose their signing ability.
    #[test]
    fn identities_sign_with_real_keys_and_can_fail() {
        let (a, b) = (TestIdentity::new(1), TestIdentity::new(2));
        assert_ne!(a.identity(), b.identity());
        let sig = a.sign(b"hello").unwrap();
        assert!(signature::verify(&a.public_key(), b"hello", &sig.0, 100).is_ok());
        assert!(signature::verify(&b.public_key(), b"hello", &sig.0, 100).is_err());
        assert!(signature::verify(&a.public_key(), b"hullo", &sig.0, 100).is_err());
        assert_eq!(a.identity().0, credential::identity_id(&a.key()).unwrap());
        a.break_signing();
        assert_eq!(a.sign(b"x"), Err(Error::Unavailable));
    }
}
