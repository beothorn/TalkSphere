//! Deterministic Storage dependency scripts; excluded from production and never advertised.
use super::contract::{Clock, ContentHash, Crypto, LeaseId, Persistence, Time};
use crate::modules::services::Error;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

pub struct ScriptClock(pub Mutex<VecDeque<Result<Time, Error>>>);
impl Clock for ScriptClock {
    // Consume exactly one coherent observation; exhaustion cannot fabricate elapsed time.
    fn now(&self) -> Result<Time, Error> {
        self.0
            .lock()
            .map_err(|_| Error::Unavailable)?
            .pop_front()
            .unwrap_or(Err(Error::Unavailable))
    }
}
type HashReply = (Vec<u8>, Result<ContentHash, Error>);

pub struct ScriptCrypto {
    pub ids: Mutex<VecDeque<Result<LeaseId, Error>>>,
    pub hashes: Mutex<VecDeque<HashReply>>,
}
impl Crypto for ScriptCrypto {
    // Scripted candidates allow exact collision and entropy-failure scenarios.
    fn lease_id(&self) -> Result<LeaseId, Error> {
        self.ids
            .lock()
            .map_err(|_| Error::Unavailable)?
            .pop_front()
            .unwrap_or(Err(Error::Unavailable))
    }
    // Assert the actual requested bytes before returning independently scripted evidence.
    fn hash(&self, bytes: &[u8]) -> Result<ContentHash, Error> {
        let (expected, result) = self
            .hashes
            .lock()
            .map_err(|_| Error::Unavailable)?
            .pop_front()
            .ok_or(Error::Unavailable)?;
        if expected != bytes {
            return Err(Error::Invalid);
        }
        result
    }
}
#[derive(Clone, Copy)]
pub enum Write {
    Commit,
    Fail,
    LoseReply,
}
#[derive(Default)]
pub struct Durable {
    pub bytes: Option<Vec<u8>>,
    pub commits: usize,
}
pub struct Store {
    pub durable: Arc<Mutex<Durable>>,
    pub writes: Mutex<VecDeque<Write>>,
}
impl Persistence for Store {
    // Read only acknowledged durable state, bounded before cloning or publication.
    fn load(&self, maximum_bytes: usize) -> Result<Option<Vec<u8>>, Error> {
        let state = self.durable.lock().map_err(|_| Error::Unavailable)?;
        if state
            .bytes
            .as_ref()
            .is_some_and(|b| b.len() > maximum_bytes)
        {
            return Err(Error::Limit);
        }
        Ok(state.bytes.clone())
    }
    // Model failed writes separately from a committed write whose acknowledgement was lost.
    fn replace(&self, bytes: &[u8]) -> Result<(), Error> {
        let action = self
            .writes
            .lock()
            .map_err(|_| Error::Unavailable)?
            .pop_front()
            .ok_or(Error::Unavailable)?;
        if matches!(action, Write::Fail) {
            return Err(Error::Unavailable);
        }
        let mut state = self.durable.lock().map_err(|_| Error::Unavailable)?;
        state.bytes = Some(bytes.to_vec());
        state.commits += 1;
        if matches!(action, Write::LoseReply) {
            return Err(Error::Unknown);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // A fresh adapter after reply loss observes durable commitment rather than guessed rollback.
    #[test]
    fn restart_preserves_committed_bytes_and_distinguishes_failures() {
        let durable = Arc::new(Mutex::new(Durable::default()));
        let store = Store {
            durable: durable.clone(),
            writes: Mutex::new([Write::Commit, Write::Fail, Write::LoseReply].into()),
        };
        assert_eq!(store.load(3), Ok(None));
        assert_eq!(store.replace(b"abc"), Ok(()));
        assert_eq!(store.replace(b"bad"), Err(Error::Unavailable));
        assert_eq!(store.load(3), Ok(Some(b"abc".to_vec())));
        assert_eq!(store.replace(b"xyz"), Err(Error::Unknown));
        drop(store);
        let recovered = Store {
            durable: durable.clone(),
            writes: Mutex::new(VecDeque::new()),
        };
        assert_eq!(recovered.load(2), Err(Error::Limit));
        assert_eq!(recovered.load(3), Ok(Some(b"xyz".to_vec())));
        assert_eq!(recovered.replace(b"new"), Err(Error::Unavailable));
        assert_eq!(durable.lock().unwrap().commits, 2);
    }
    // Explicit clock rollback and repeated random candidates remain visible; exhaustion fails closed.
    #[test]
    fn scripts_preserve_clock_changes_collisions_and_dependency_failures() {
        let time = Time {
            unix_seconds: 1000,
            elapsed_seconds: 5,
            boot: [1; 16],
        };
        let clock = ScriptClock(Mutex::new(
            [
                Ok(time),
                Ok(Time {
                    unix_seconds: 900,
                    elapsed_seconds: 6,
                    ..time
                }),
                Err(Error::Unavailable),
            ]
            .into(),
        ));
        assert_eq!(clock.now(), Ok(time));
        assert_eq!(clock.now().unwrap().unix_seconds, 900);
        assert_eq!(clock.now(), Err(Error::Unavailable));
        assert_eq!(clock.now(), Err(Error::Unavailable));
        let crypto = ScriptCrypto {
            ids: Mutex::new([Ok([2; 32]), Ok([2; 32]), Err(Error::Unavailable)].into()),
            hashes: Mutex::new(
                [
                    (b"abc".to_vec(), Ok([3; 32])),
                    (b"abc".to_vec(), Err(Error::Unavailable)),
                    (b"abc".to_vec(), Ok([3; 32])),
                ]
                .into(),
            ),
        };
        assert_eq!(crypto.lease_id(), Ok([2; 32]));
        assert_eq!(crypto.lease_id(), Ok([2; 32]));
        assert_eq!(crypto.lease_id(), Err(Error::Unavailable));
        assert_eq!(crypto.lease_id(), Err(Error::Unavailable));
        assert_eq!(crypto.hash(b"abc"), Ok([3; 32]));
        assert_eq!(crypto.hash(b"abc"), Err(Error::Unavailable));
        assert_eq!(crypto.hash(b"wrong"), Err(Error::Invalid));
        assert_eq!(crypto.hash(b"abc"), Err(Error::Unavailable));
    }
}

// Bounded dependency reads model partial chunks, dependency loss and premature EOF without allocation by declared size.
#[test]
fn bounded_source_and_concurrent_writes_are_deterministic() {
    use std::{
        io::{self, Read},
        sync::Barrier,
        thread,
    };
    struct Source(VecDeque<io::Result<Vec<u8>>>);
    impl Read for Source {
        // Never return a larger read than the caller's buffer; preserve the remaining fragment.
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            if out.is_empty() {
                return Ok(0);
            }
            let Some(chunk) = self.0.pop_front() else {
                return Ok(0);
            };
            let bytes = chunk?;
            let count = bytes.len().min(out.len());
            out[..count].copy_from_slice(&bytes[..count]);
            if count < bytes.len() {
                self.0.push_front(Ok(bytes[count..].to_vec()));
            }
            Ok(count)
        }
    }
    let mut source = Source(
        [
            Ok(b"abc".to_vec()),
            Err(io::Error::other("injected read failure")),
        ]
        .into(),
    );
    let mut byte = [0];
    for expected in b"abc" {
        assert_eq!(source.read(&mut byte).unwrap(), 1);
        assert_eq!(byte[0], *expected);
    }
    assert!(source.read(&mut byte).is_err());
    assert_eq!(source.read(&mut byte).unwrap(), 0);
    let gate = Arc::new(Barrier::new(3));
    let durable = Arc::new(Mutex::new(Durable::default()));
    let joins: Vec<_> = (0..2)
        .map(|value| {
            let gate = gate.clone();
            let durable = durable.clone();
            thread::spawn(move || {
                let store = Store {
                    durable,
                    writes: Mutex::new([Write::Commit].into()),
                };
                gate.wait();
                store.replace(&[value]).unwrap();
            })
        })
        .collect();
    gate.wait();
    for join in joins {
        join.join().unwrap();
    }
    assert_eq!(durable.lock().unwrap().commits, 2);
    assert!(matches!(
        durable.lock().unwrap().bytes.as_deref(),
        Some([0]) | Some([1])
    ));
}

pub(crate) mod contracts;

#[cfg(target_os = "linux")]
pub(crate) mod ranges;
