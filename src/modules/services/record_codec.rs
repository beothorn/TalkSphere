//! Bounded private Services image framing, not a peer wire format.
use super::{Error, contract::Limits, persistence::Record};
use crate::modules::identity::api::IdentityId;
// Include room for fixed framing without allowing unchecked allocation arithmetic.
pub(super) fn maximum(limits: Limits) -> Result<usize, Error> {
    tracing::trace!("Checking Services image length budget");
    limits.validate()?;
    limits.state_bytes.checked_add(48).ok_or(Error::Limit)
}
// Encode exact private framing only after checking the complete payload bound.
pub(super) fn encode(record: &Record, limits: Limits) -> Result<Vec<u8>, Error> {
    tracing::trace!("Encoding complete Services image");
    maximum(limits)?;
    if record.payload.len() > limits.state_bytes {
        return Err(Error::Limit);
    }
    let mut out = b"TSSERVC1".to_vec();
    out.extend_from_slice(&record.identity.0);
    out.extend_from_slice(&(record.payload.len() as u64).to_be_bytes());
    out.extend_from_slice(&record.payload);
    Ok(out)
}
// Require exact framing/version/length; unknown schemas are never silently reinitialized.
pub(super) fn decode(bytes: &[u8], limits: Limits) -> Result<Record, Error> {
    tracing::trace!("Decoding bounded Services image");
    if bytes.len() > maximum(limits)? {
        return Err(Error::Limit);
    }
    if bytes.len() < 48 || &bytes[..8] != b"TSSERVC1" {
        return Err(Error::Invalid);
    }
    let length = u64::from_be_bytes(bytes[40..48].try_into().map_err(|_| Error::Invalid)?);
    if length > limits.state_bytes as u64 {
        return Err(Error::Limit);
    }
    if length != (bytes.len() - 48) as u64 {
        return Err(Error::Invalid);
    }
    Ok(Record {
        identity: IdentityId(bytes[8..40].try_into().map_err(|_| Error::Invalid)?),
        payload: bytes[48..].to_vec(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    // Literal framing rejects partial, oversized, trailing and future-version images before allocation.
    #[test]
    fn framing_boundaries() {
        let l = Limits {
            handlers: 1,
            operations: 1,
            bodies: 1,
            nodes: 1,
            quotes: 1,
            state_bytes: 1,
        };
        let r = Record {
            identity: IdentityId([3; 32]),
            payload: vec![9],
        };
        let bytes = encode(&r, l).unwrap();
        assert_eq!(&bytes[..8], b"TSSERVC1");
        assert_eq!(&bytes[40..48], &1u64.to_be_bytes());
        assert_eq!(decode(&bytes, l), Ok(r.clone()));
        for n in 0..bytes.len() {
            assert!(decode(&bytes[..n], l).is_err())
        }
        let mut bad = bytes.clone();
        bad[7] = b'2';
        assert_eq!(decode(&bad, l), Err(Error::Invalid));
        bad = bytes.clone();
        bad.push(0);
        assert!(decode(&bad, l).is_err());
        bad = bytes;
        bad[40..48].copy_from_slice(&u64::MAX.to_be_bytes());
        assert_eq!(decode(&bad, l), Err(Error::Limit));
        assert_eq!(
            encode(
                &Record {
                    payload: vec![0; 2],
                    ..r
                },
                l
            ),
            Err(Error::Limit)
        );
    }
}
