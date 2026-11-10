//! Lossless bounded wire JSON control documents with duplicate-member and depth preflight.
use super::{Error, record::CONTROL_MAX};
use serde_json::Value;
use std::collections::BTreeSet;
/// Exact control source and immutable lossless JSON; raw bytes remain available for quote hashing.
#[derive(Clone)]
pub struct Control {
    bytes: Vec<u8>,
    value: Value,
}
impl Control {
    /// Parse one complete object with a nonempty type after bounded UTF-8/structure validation.
    /// # Errors
    /// Invalid rejects syntax, duplicates, trailing input, excessive depth and missing/wrong type; Limit rejects byte/node capacity.
    pub fn parse(bytes: &[u8], nodes: usize) -> Result<Self, Error> {
        tracing::trace!("Validating lossless bounded peer control JSON");
        if bytes.is_empty() || bytes.len() > CONTROL_MAX || nodes == 0 {
            return Err(Error::Limit);
        }
        std::str::from_utf8(bytes).map_err(|_| Error::Invalid)?;
        let mut scan = Scan {
            bytes,
            pos: 0,
            nodes,
        };
        scan.value(0)?;
        scan.space();
        if scan.pos != bytes.len() {
            return Err(Error::Invalid);
        }
        let value: Value = serde_json::from_slice(bytes).map_err(|_| Error::Invalid)?;
        if !value.is_object()
            || !value
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|s| !s.is_empty())
        {
            return Err(Error::Invalid);
        }
        Ok(Self {
            bytes: bytes.to_vec(),
            value,
        })
    }
    /// Borrow a top-level field's exact JSON source, including adjacent value whitespace, without reserialization.
    /// # Errors
    /// Returns None for absent fields; malformed internal structure remains an explicit failure.
    pub fn field_bytes(&self, name: &str) -> Result<Option<&[u8]>, Error> {
        tracing::trace!("Borrowing exact peer control subdocument source");
        let mut scan = Scan {
            bytes: &self.bytes,
            pos: 0,
            nodes: self.bytes.len(),
        };
        scan.space();
        if scan.bytes.get(scan.pos) != Some(&b'{') {
            return Err(Error::Invalid);
        }
        scan.pos += 1;
        loop {
            scan.space();
            if scan.bytes.get(scan.pos) == Some(&b'}') {
                return Ok(None);
            }
            let key = scan.string()?;
            scan.space();
            if scan.bytes.get(scan.pos) != Some(&b':') {
                return Err(Error::Invalid);
            }
            scan.pos += 1;
            let start = scan.pos;
            scan.value(1)?;
            scan.space();
            let end = scan.pos;
            if key == name {
                return Ok(Some(&self.bytes[start..end]));
            }
            match scan.bytes.get(scan.pos) {
                Some(b',') => scan.pos += 1,
                Some(b'}') => return Ok(None),
                _ => return Err(Error::Invalid),
            }
        }
    }
    /// Borrow exact original JSON body bytes without reserialization.
    pub fn bytes(&self) -> &[u8] {
        tracing::trace!("Borrowing exact peer control source");
        &self.bytes
    }
    /// Borrow exact arbitrary-precision JSON values; later message validators enforce field-specific forms.
    pub fn value(&self) -> &Value {
        tracing::trace!("Borrowing lossless peer control fields");
        &self.value
    }
}
struct Scan<'a> {
    bytes: &'a [u8],
    pos: usize,
    nodes: usize,
}
impl Scan<'_> {
    // Skip only JSON's four whitespace bytes, never Unicode whitespace or BOM.
    fn space(&mut self) {
        while self
            .bytes
            .get(self.pos)
            .is_some_and(|b| matches!(b, b' ' | b'\n' | b'\r' | b'\t'))
        {
            self.pos += 1;
        }
    }
    // Decode a bounded quoted token so escaped equivalent object names collide.
    fn string(&mut self) -> Result<String, Error> {
        let start = self.pos;
        if self.bytes.get(self.pos) != Some(&b'"') {
            return Err(Error::Invalid);
        }
        self.pos += 1;
        loop {
            match self.bytes.get(self.pos) {
                Some(b'"') => {
                    self.pos += 1;
                    break;
                }
                Some(b'\\') => {
                    self.pos = self.pos.checked_add(2).ok_or(Error::Invalid)?;
                }
                Some(_) => self.pos += 1,
                None => return Err(Error::Invalid),
            }
        }
        serde_json::from_slice(&self.bytes[start..self.pos]).map_err(|_| Error::Invalid)
    }
    // Bound nesting and nodes before constructing the tree; per-byte traversal deliberately emits no logs.
    fn value(&mut self, depth: usize) -> Result<(), Error> {
        self.space();
        self.nodes = self.nodes.checked_sub(1).ok_or(Error::Limit)?;
        let first = *self.bytes.get(self.pos).ok_or(Error::Invalid)?;
        if first == b'{' || first == b'[' {
            if depth >= 32 {
                return Err(Error::Invalid);
            }
            self.pos += 1;
            self.space();
            let object = first == b'{';
            let end = if object { b'}' } else { b']' };
            let mut names = BTreeSet::new();
            if self.bytes.get(self.pos) == Some(&end) {
                self.pos += 1;
                return Ok(());
            }
            loop {
                self.space();
                if object {
                    let name = self.string()?;
                    if !names.insert(name) {
                        return Err(Error::Invalid);
                    }
                    self.space();
                    if self.bytes.get(self.pos) != Some(&b':') {
                        return Err(Error::Invalid);
                    }
                    self.pos += 1;
                }
                self.value(depth + 1)?;
                self.space();
                match self.bytes.get(self.pos) {
                    Some(b',') => self.pos += 1,
                    Some(b) if *b == end => {
                        self.pos += 1;
                        break;
                    }
                    _ => return Err(Error::Invalid),
                }
            }
            return Ok(());
        }
        if first == b'"' {
            self.string()?;
            return Ok(());
        }
        let start = self.pos;
        while self
            .bytes
            .get(self.pos)
            .is_some_and(|b| !matches!(b, b' ' | b'\n' | b'\r' | b'\t' | b',' | b'}' | b']'))
        {
            self.pos += 1;
        }
        if start == self.pos {
            return Err(Error::Invalid);
        }
        let _: Value =
            serde_json::from_slice(&self.bytes[start..self.pos]).map_err(|_| Error::Invalid)?;
        Ok(())
    }
}
