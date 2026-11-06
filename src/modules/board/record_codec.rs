//! Bounded private record encoding; this format never appears on CR2SE peer streams.
use super::{Error, config::Limits, persistence::Record};
use crate::modules::identity::api::IdentityId;
use std::collections::BTreeMap;

// Calculate the bounded record envelope including repeated lookup IDs and length fields.
pub(super) fn maximum(l: Limits) -> Result<usize, Error> {
    tracing::trace!("Calculating private Board record bounds");
    l.publication_bytes
        .checked_mul(2)
        .and_then(|n| n.checked_add(l.definitions.checked_mul(12)?))
        .and_then(|n| n.checked_add(64))
        .ok_or(Error::Limit)
}
// Serialize only bounded complete records, without treating stored metadata as authority.
pub(super) fn encode(r: &Record, l: Limits) -> Result<Vec<u8>, Error> {
    tracing::trace!("Encoding a bounded private Board record");
    l.validate()?;
    if r.definitions.len() > l.definitions {
        return Err(Error::Limit);
    }
    let total = r.definitions.values().try_fold(r.board.len(), |n, d| {
        n.checked_add(d.len()).ok_or(Error::Limit)
    })?;
    if total > l.publication_bytes {
        return Err(Error::Limit);
    }
    let mut out = b"TSBOARD1".to_vec();
    out.extend_from_slice(&r.publisher.0);
    put(&mut out, &r.board, l.document_bytes)?;
    out.extend_from_slice(
        &u32::try_from(r.definitions.len())
            .map_err(|_| Error::Limit)?
            .to_be_bytes(),
    );
    for (id, d) in &r.definitions {
        put(&mut out, id.as_bytes(), l.string_bytes)?;
        put(&mut out, d, l.document_bytes)?;
    }
    if out.len() > maximum(l)? {
        return Err(Error::Limit);
    }
    Ok(out)
}
// Decode length-delimited bytes after checking the complete envelope budget.
pub(super) fn decode(bytes: &[u8], l: Limits) -> Result<Record, Error> {
    tracing::trace!("Decoding a complete private Board record");
    l.validate()?;
    if bytes.len() > maximum(l)? {
        return Err(Error::Limit);
    }
    let mut rest = bytes;
    if take(&mut rest, 8)? != b"TSBOARD1" {
        return Err(Error::Invalid);
    }
    let publisher = IdentityId(
        take(&mut rest, 32)?
            .try_into()
            .map_err(|_| Error::Invalid)?,
    );
    let board = field(&mut rest, l.document_bytes)?.to_vec();
    let count = number(&mut rest)?;
    if count > l.definitions {
        return Err(Error::Limit);
    }
    let mut definitions = BTreeMap::new();
    let mut total = board.len();
    for _ in 0..count {
        let id = std::str::from_utf8(field(&mut rest, l.string_bytes)?)
            .map_err(|_| Error::Invalid)?
            .to_owned();
        let d = field(&mut rest, l.document_bytes)?;
        total = total.checked_add(d.len()).ok_or(Error::Limit)?;
        if total > l.publication_bytes {
            return Err(Error::Limit);
        }
        if definitions.insert(id, d.to_vec()).is_some() {
            return Err(Error::Invalid);
        }
    }
    if !rest.is_empty() {
        return Err(Error::Invalid);
    }
    Ok(Record {
        publisher,
        board,
        definitions,
    })
}
// Prefix a bounded field with its exact uint32 length.
fn put(out: &mut Vec<u8>, bytes: &[u8], maximum: usize) -> Result<(), Error> {
    tracing::trace!("Writing a bounded private record field");
    if bytes.len() > maximum {
        return Err(Error::Limit);
    }
    out.extend_from_slice(
        &u32::try_from(bytes.len())
            .map_err(|_| Error::Limit)?
            .to_be_bytes(),
    );
    out.extend_from_slice(bytes);
    Ok(())
}
// Borrow exactly n bytes without panicking on truncated input.
fn take<'a>(rest: &mut &'a [u8], n: usize) -> Result<&'a [u8], Error> {
    tracing::trace!("Reading a fixed-width private record field");
    if rest.len() < n {
        return Err(Error::Invalid);
    }
    let (v, tail) = rest.split_at(n);
    *rest = tail;
    Ok(v)
}
// Read an exact big-endian uint32 length or count.
fn number(rest: &mut &[u8]) -> Result<usize, Error> {
    tracing::trace!("Reading a private record length");
    Ok(u32::from_be_bytes(take(rest, 4)?.try_into().map_err(|_| Error::Invalid)?) as usize)
}
// Reject an excessive length before borrowing or allocating its payload.
fn field<'a>(rest: &mut &'a [u8], maximum: usize) -> Result<&'a [u8], Error> {
    tracing::trace!("Reading bounded private record payload");
    let n = number(rest)?;
    if n > maximum {
        return Err(Error::Limit);
    }
    take(rest, n)
}
#[cfg(test)]
mod tests {
    use super::*;
    // Every truncation is rejected and exact source bytes survive independent private serialization.
    #[test]
    fn private_record_boundaries() {
        let l = Limits::default();
        let r = Record {
            publisher: IdentityId([1; 32]),
            board: b"{}".to_vec(),
            definitions: BTreeMap::from([("x".into(), b"{}".to_vec())]),
        };
        let bytes = encode(&r, l).unwrap();
        assert_eq!(decode(&bytes, l), Ok(r));
        for i in 0..bytes.len() {
            assert!(decode(&bytes[..i], l).is_err());
        }
        let mut bad = bytes.clone();
        bad.push(0);
        assert!(decode(&bad, l).is_err());
        bad = bytes;
        bad[0] = 0;
        assert!(decode(&bad, l).is_err());
    }
}
