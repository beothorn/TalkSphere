//! Versioned identity-bound Storage metadata images; exact bytes and results survive owner restart.
use super::{
    contract::{Lease, State, Time},
    retention::Anchor,
};
use crate::modules::{
    cryptography::primitives::hash::sha256,
    identity::api::IdentityId,
    services::{
        self, Error,
        contract::OperationKey,
        handler::{Blob, Completion},
        value::Budget,
    },
};
use std::collections::BTreeSet;

/// Finite independent persistence limits, separate from peer-advertised operation limits.
#[derive(Clone, Copy)]
pub struct Limits {
    /// Maximum complete encoded image bytes, including framing and checksum.
    pub bytes: usize,
    /// Maximum retained lease identities, including terminal records.
    pub leases: usize,
    /// Maximum retained original operation records.
    pub operations: usize,
    /// Independent structural/content bounds for each logical result.
    pub values: Budget,
}
/// Durable family-local phase, including staged work before Services preparation.
#[derive(Clone)]
pub enum Stage {
    /// Durable proposed effect/output with no preparation token yet.
    Staged,
    /// Exact owner-issued token retained before acknowledging preparation.
    Prepared(Blob),
    /// Immutable coordinator commit was applied; retain the exact token for duplicate validation.
    Committed(Blob),
    /// Immutable coordinator abort was applied; retain a preparation token when one existed.
    Aborted(Option<Blob>),
}
/// Retained completion and participant status for one original local attempt.
#[derive(Clone)]
pub struct Retained {
    /// Trusted original requester/connection/stream association.
    pub key: OperationKey,
    /// Digest of exact accepted input/terms used to reject changed local reuse.
    pub binding: [u8; 32],
    /// Complete durable output, result and proposed effect, never a session-owned reference.
    pub completion: Completion,
    /// Family-local durable phase; this does not change Services or peer status values.
    pub status: Stage,
}
/// Complete owner metadata; byte object IDs equal their corresponding lease IDs.
#[derive(Clone)]
pub struct Image {
    /// Authoritative provider identity, checked independently of filesystem location.
    pub provider: IdentityId,
    /// Correction-resistant last acknowledged retention observation.
    pub anchor: Anchor,
    /// Committed lease descriptions, including terminal ID history.
    pub leases: Vec<Lease>,
    /// Retained operation effects, exact outputs and durable lifecycle status.
    pub operations: Vec<Retained>,
}
const MAGIC: &[u8; 8] = b"TSSTORE1";
impl Image {
    /// Encode a bounded exact image after validating ownership, unique keys and lease invariants.
    /// # Errors
    /// Invalid rejects inconsistent state; Limit rejects configured count/content/encoding bounds.
    pub fn encode(&self, limits: Limits) -> Result<Vec<u8>, Error> {
        tracing::trace!("Encoding complete identity-bound Storage metadata");
        validate(self, limits)?;
        let mut w = Writer {
            bytes: Vec::new(),
            maximum: limits.bytes.checked_sub(32).ok_or(Error::Limit)?,
        };
        w.put(MAGIC)?;
        w.put(&self.provider.0)?;
        for n in [
            self.anchor.unix_seconds,
            self.anchor.observation.unix_seconds,
            self.anchor.observation.elapsed_seconds,
        ] {
            w.number(n)?;
        }
        w.put(&self.anchor.observation.boot)?;
        w.count(self.leases.len())?;
        for l in &self.leases {
            w.put(&l.id)?;
            w.put(&l.requester.0)?;
            w.put(&l.provider.0)?;
            w.blob(l.accepted_terms.bytes())?;
            w.number(l.size_bytes)?;
            w.put(&l.content_hash)?;
            for n in [l.accepted_at, l.expires_at, l.store_price] {
                w.number(n)?;
            }
            w.put(&[match l.state {
                State::Active => 1,
                State::Expired => 2,
                State::Removed => 3,
                State::Receiving => return Err(Error::Invalid),
            }])?;
        }
        w.count(self.operations.len())?;
        for o in &self.operations {
            w.put(&o.key.requester.0)?;
            w.put(&o.key.connection)?;
            w.put(&o.key.stream.to_be_bytes())?;
            w.put(&o.binding)?;
            let output = services::codec::encode(
                &o.completion.output,
                limits.values,
                w.maximum - w.bytes.len(),
            )?;
            w.blob(&output)?;
            w.blob(o.completion.result.bytes())?;
            w.blob(o.completion.effect.bytes())?;
            w.number(o.completion.price)?;
            match &o.status {
                Stage::Staged => w.put(&[0])?,
                Stage::Prepared(token) => {
                    w.put(&[1])?;
                    w.blob(token.bytes())?;
                }
                Stage::Committed(token) => {
                    w.put(&[2])?;
                    w.blob(token.bytes())?;
                }
                Stage::Aborted(token) => {
                    w.put(&[3])?;
                    match token {
                        None => w.put(&[0])?,
                        Some(token) => {
                            w.put(&[1])?;
                            w.blob(token.bytes())?;
                        }
                    }
                }
            }
        }
        let hash = sha256(&w.bytes, w.maximum).map_err(|_| Error::Limit)?;
        w.bytes.extend_from_slice(&hash);
        Ok(w.bytes)
    }
    /// Decode only a complete valid image for the expected local owner, preserving exact integer widths.
    /// # Errors
    /// Invalid rejects corruption, version/type/truncation/duplicate errors; Denied rejects a foreign owner.
    /// Limit rejects encoded, collection or logical result budgets before proportional allocation.
    pub fn decode(bytes: &[u8], provider: IdentityId, limits: Limits) -> Result<Self, Error> {
        tracing::trace!("Decoding bounded durable Storage metadata");
        check_limits(limits)?;
        if bytes.len() > limits.bytes {
            return Err(Error::Limit);
        }
        let split = bytes.len().checked_sub(32).ok_or(Error::Invalid)?;
        let (body, digest) = bytes.split_at(split);
        if sha256(body, limits.bytes)
            .map_err(|_| Error::Limit)?
            .as_slice()
            != digest
        {
            return Err(Error::Invalid);
        }
        let mut r = Reader(body);
        if r.take(8)? != MAGIC {
            return Err(Error::Invalid);
        }
        if IdentityId(r.fixed()?) != provider {
            return Err(Error::Denied);
        }
        let unix_seconds = r.number()?;
        let wall = r.number()?;
        let elapsed = r.number()?;
        let boot = r.fixed()?;
        let anchor = Anchor {
            unix_seconds,
            observation: Time {
                unix_seconds: wall,
                elapsed_seconds: elapsed,
                boot,
            },
        };
        let count = r.count(limits.leases)?;
        let mut leases = Vec::new();
        for _ in 0..count {
            let id = r.fixed()?;
            let requester = IdentityId(r.fixed()?);
            let local = IdentityId(r.fixed()?);
            let accepted_terms = r.blob(limits.bytes)?;
            let size_bytes = r.number()?;
            let content_hash = r.fixed()?;
            let accepted_at = r.number()?;
            let expires_at = r.number()?;
            let store_price = r.number()?;
            let state = match r.take(1)?[0] {
                1 => State::Active,
                2 => State::Expired,
                3 => State::Removed,
                _ => return Err(Error::Invalid),
            };
            leases.push(Lease {
                id,
                requester,
                provider: local,
                accepted_terms,
                size_bytes,
                content_hash,
                accepted_at,
                expires_at,
                store_price,
                state,
            });
        }
        let count = r.count(limits.operations)?;
        let mut operations = Vec::new();
        for _ in 0..count {
            let key = OperationKey {
                requester: IdentityId(r.fixed()?),
                connection: r.fixed()?,
                stream: u32::from_be_bytes(r.fixed()?),
            };
            let binding = r.fixed()?;
            let output = services::codec::decode(r.bytes()?, limits.values, limits.bytes)?;
            let result = r.blob(limits.bytes)?;
            let effect = r.blob(limits.bytes)?;
            let price = r.number()?;
            let status = match r.take(1)?[0] {
                0 => Stage::Staged,
                1 => Stage::Prepared(r.blob(limits.bytes)?),
                2 => Stage::Committed(r.blob(limits.bytes)?),
                3 => Stage::Aborted(match r.take(1)?[0] {
                    0 => None,
                    1 => Some(r.blob(limits.bytes)?),
                    _ => return Err(Error::Invalid),
                }),
                _ => return Err(Error::Invalid),
            };
            operations.push(Retained {
                key,
                binding,
                completion: Completion {
                    output,
                    result,
                    effect,
                    price,
                },
                status,
            });
        }
        if !r.0.is_empty() {
            return Err(Error::Invalid);
        }
        let image = Self {
            provider,
            anchor,
            leases,
            operations,
        };
        validate(&image, limits)?;
        Ok(image)
    }
}
// Reject invalid capacities without allocating collections from untrusted counts.
fn check_limits(l: Limits) -> Result<(), Error> {
    tracing::trace!("Checking finite Storage metadata limits");
    if l.bytes < 128 || l.leases == 0 || l.operations == 0 {
        return Err(Error::Invalid);
    }
    Ok(())
}
// Owner metadata cannot silently change identity, reuse keys, or reactivate incomplete leases.
fn validate(image: &Image, limits: Limits) -> Result<(), Error> {
    tracing::trace!("Validating complete Storage owner metadata invariants");
    check_limits(limits)?;
    if image.leases.len() > limits.leases || image.operations.len() > limits.operations {
        return Err(Error::Limit);
    }
    let mut ids = BTreeSet::new();
    for l in &image.leases {
        if l.provider != image.provider
            || !ids.insert(l.id)
            || l.size_bytes == 0
            || l.store_price == 0
            || l.expires_at <= l.accepted_at
            || l.accepted_terms.bytes().is_empty()
            || l.state == State::Receiving
        {
            return Err(Error::Invalid);
        }
    }
    let mut keys = BTreeSet::new();
    for o in &image.operations {
        if !keys.insert(o.key)
            || o.completion.price == 0
            || matches!(&o.status,Stage::Prepared(t)|Stage::Committed(t)|Stage::Aborted(Some(t)) if t.bytes().is_empty())
        {
            return Err(Error::Invalid);
        }
        o.completion.output.bounded(limits.values)?;
    }
    Ok(())
}
struct Writer {
    bytes: Vec<u8>,
    maximum: usize,
}
impl Writer {
    // Check the next complete segment before extending the encoded owner image.
    fn put(&mut self, bytes: &[u8]) -> Result<(), Error> {
        tracing::trace!("Appending bounded Storage metadata segment");
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or(Error::Limit)?
            > self.maximum
        {
            return Err(Error::Limit);
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    // Preserve exact uint64 values as fixed big-endian local integers.
    fn number(&mut self, n: u64) -> Result<(), Error> {
        tracing::trace!("Encoding exact Storage metadata integer");
        self.put(&n.to_be_bytes())
    }
    // Bound local collection/blob counts to the private format's uint32 width.
    fn count(&mut self, n: usize) -> Result<(), Error> {
        tracing::trace!("Encoding bounded Storage metadata count");
        self.put(&u32::try_from(n).map_err(|_| Error::Limit)?.to_be_bytes())
    }
    // Copy only an admitted length-prefixed owner byte field.
    fn blob(&mut self, b: &[u8]) -> Result<(), Error> {
        tracing::trace!("Encoding retained Storage metadata bytes");
        self.count(b.len())?;
        self.put(b)
    }
}
struct Reader<'a>(&'a [u8]);
impl<'a> Reader<'a> {
    // Borrow only bytes present in the bounded complete input, rejecting truncation before copying.
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        tracing::trace!("Reading bounded Storage metadata segment");
        if n > self.0.len() {
            return Err(Error::Invalid);
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    // Require exact fixed-width local scalar representations.
    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        tracing::trace!("Reading fixed Storage metadata field");
        self.take(N)?.try_into().map_err(|_| Error::Invalid)
    }
    // Preserve uint64 without a lossy host-number conversion.
    fn number(&mut self) -> Result<u64, Error> {
        tracing::trace!("Reading exact Storage metadata integer");
        Ok(u64::from_be_bytes(self.fixed()?))
    }
    // Reject collection counts above the caller's independent bound before building a collection.
    fn count(&mut self, max: usize) -> Result<usize, Error> {
        tracing::trace!("Reading bounded Storage metadata count");
        let n = u32::from_be_bytes(self.fixed()?) as usize;
        if n > max {
            return Err(Error::Limit);
        }
        Ok(n)
    }
    // Borrow a length-prefixed byte field only after verifying it actually exists.
    fn bytes(&mut self) -> Result<&'a [u8], Error> {
        tracing::trace!("Reading retained Storage metadata byte slice");
        let n = self.count(self.0.len())?;
        self.take(n)
    }
    // Retain a complete independent Blob rather than a process-local file path or pointer.
    fn blob(&mut self, max: usize) -> Result<Blob, Error> {
        tracing::trace!("Retaining decoded Storage owner bytes");
        let b = self.bytes()?;
        if b.len() > max {
            return Err(Error::Limit);
        }
        Blob::new(b.to_vec(), max)
    }
}

#[cfg(target_os = "linux")]
impl Image {
    /// Verify active byte objects against retained size/hash before exposing recovered leases.
    /// Reads use at most 65536 bytes per chunk; terminal lease metadata may outlive physical bytes.
    /// # Errors
    /// Missing, short or mismatching data fails Storage/Invalid; oversized hashing bounds fail Limit.
    /// Callers must gate affected obligations rather than silently dropping them or reinitializing state.
    pub fn verify_active(&self, store: &super::bytes::Store) -> Result<(), Error> {
        tracing::trace!("Checking retained Storage bytes before recovery admission");
        use crate::modules::cryptography::primitives::hash::Sha256State;
        for lease in &self.leases {
            if lease.state != State::Active {
                continue;
            }
            let handle = store.reopen(lease.id, lease.size_bytes)?;
            let mut hash =
                Sha256State::new(usize::try_from(lease.size_bytes).map_err(|_| Error::Limit)?);
            let mut offset = 0;
            while offset < lease.size_bytes {
                let length = (lease.size_bytes - offset).min(65536);
                let chunk = handle.read(offset, length, 65536)?;
                hash.update(&chunk).map_err(|_| Error::Limit)?;
                offset += length;
            }
            if hash.finish() != lease.content_hash {
                return Err(Error::Storage);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/image.rs"]
mod tests;
