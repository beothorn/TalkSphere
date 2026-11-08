//! Bounded private/local value encoding; never a CR2SE peer wire format.
use super::{
    Error,
    value::{Budget, Value},
};
/// Encode exact logical values under finite structural and encoded-byte limits.
/// # Errors
/// Invalid objects and exhausted limits fail without returning a partial encoding.
pub fn encode(value: &Value, budget: Budget, maximum: usize) -> Result<Vec<u8>, Error> {
    tracing::trace!("Encoding bounded local logical service value");
    value.bounded(budget)?;
    let mut out = Writer {
        bytes: Vec::new(),
        maximum,
    };
    write(value, &mut out)?;
    Ok(out.bytes)
}
/// Decode exact local values with bounded allocation, nesting and node counts.
/// # Errors
/// Unknown tags, malformed UTF-8, trailing/truncated bytes and exhausted budgets fail explicitly.
pub fn decode(bytes: &[u8], budget: Budget, maximum: usize) -> Result<Value, Error> {
    tracing::trace!("Decoding bounded local logical service value");
    if bytes.len() > maximum {
        return Err(Error::Limit);
    }
    let mut r = Reader {
        bytes,
        nodes: budget.nodes,
        depth: budget.depth.min(31),
        remaining: budget.bytes,
    };
    let value = r.value(0)?;
    if !r.bytes.is_empty() {
        return Err(Error::Invalid);
    }
    value.bounded(budget)?;
    Ok(value)
}
struct Writer {
    bytes: Vec<u8>,
    maximum: usize,
}
impl Writer {
    // Reserve only checked encoded capacity and never return a proportional allocation before admission.
    fn put(&mut self, b: &[u8]) -> Result<(), Error> {
        if self.bytes.len().checked_add(b.len()).ok_or(Error::Limit)? > self.maximum {
            return Err(Error::Limit);
        }
        self.bytes.extend_from_slice(b);
        Ok(())
    }
    // Length-prefix a local byte sequence with a checked uint32 size.
    fn blob(&mut self, b: &[u8]) -> Result<(), Error> {
        self.put(
            &u32::try_from(b.len())
                .map_err(|_| Error::Limit)?
                .to_be_bytes(),
        )?;
        self.put(b)
    }
}
// Tagged logical values preserve integer widths and IEEE bits with no JSON interpretation.
fn write(v: &Value, w: &mut Writer) -> Result<(), Error> {
    match v {
        Value::Bool(b) => {
            w.put(&[0, u8::from(*b)])?;
        }
        Value::Int32(n) => {
            w.put(&[1])?;
            w.put(&n.to_be_bytes())?;
        }
        Value::Int64(n) => {
            w.put(&[2])?;
            w.put(&n.to_be_bytes())?;
        }
        Value::Uint32(n) => {
            w.put(&[3])?;
            w.put(&n.to_be_bytes())?;
        }
        Value::Uint64(n) => {
            w.put(&[4])?;
            w.put(&n.to_be_bytes())?;
        }
        Value::Float(n) => {
            w.put(&[5])?;
            w.put(&n.to_be_bytes())?;
        }
        Value::Double(n) => {
            w.put(&[6])?;
            w.put(&n.to_be_bytes())?;
        }
        Value::String(s) => {
            w.put(&[7])?;
            w.blob(s.as_bytes())?;
        }
        Value::Bytes(b) => {
            w.put(&[8])?;
            w.blob(b)?;
        }
        Value::Object(fields) => {
            w.put(&[9])?;
            w.put(
                &u32::try_from(fields.len())
                    .map_err(|_| Error::Limit)?
                    .to_be_bytes(),
            )?;
            for (n, v) in fields {
                w.blob(n.as_bytes())?;
                write(v, w)?;
            }
        }
        Value::Array(values) => {
            w.put(&[10])?;
            w.put(
                &u32::try_from(values.len())
                    .map_err(|_| Error::Limit)?
                    .to_be_bytes(),
            )?;
            for v in values {
                write(v, w)?;
            }
        }
    }
    Ok(())
}
struct Reader<'a> {
    bytes: &'a [u8],
    nodes: usize,
    depth: usize,
    remaining: usize,
}
impl<'a> Reader<'a> {
    // Consume only available bytes; lengths cannot trigger speculative allocation.
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        if n > self.bytes.len() {
            return Err(Error::Invalid);
        }
        let (a, b) = self.bytes.split_at(n);
        self.bytes = b;
        Ok(a)
    }
    // Read one big-endian fixed-width scalar, rejecting truncated input.
    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        self.take(N)?.try_into().map_err(|_| Error::Invalid)
    }
    // Decode a local uint32 collection length without coercion.
    fn length(&mut self) -> Result<usize, Error> {
        usize::try_from(u32::from_be_bytes(self.fixed()?)).map_err(|_| Error::Limit)
    }
    // Borrow one already present byte sequence; callers copy only after size validation.
    fn blob(&mut self) -> Result<&'a [u8], Error> {
        let n = self.length()?;
        self.remaining = self.remaining.checked_sub(n).ok_or(Error::Limit)?;
        self.take(n)
    }
    // Bound every recursive node before entering nested data or reserving collection storage.
    fn value(&mut self, depth: usize) -> Result<Value, Error> {
        if depth > self.depth {
            return Err(Error::Limit);
        }
        self.nodes = self.nodes.checked_sub(1).ok_or(Error::Limit)?;
        Ok(match self.fixed::<1>()?[0] {
            0 => Value::Bool(match self.fixed::<1>()?[0] {
                0 => false,
                1 => true,
                _ => return Err(Error::Invalid),
            }),
            1 => Value::Int32(i32::from_be_bytes(self.fixed()?)),
            2 => Value::Int64(i64::from_be_bytes(self.fixed()?)),
            3 => Value::Uint32(u32::from_be_bytes(self.fixed()?)),
            4 => Value::Uint64(u64::from_be_bytes(self.fixed()?)),
            5 => Value::Float(u32::from_be_bytes(self.fixed()?)),
            6 => Value::Double(u64::from_be_bytes(self.fixed()?)),
            7 => Value::String(
                std::str::from_utf8(self.blob()?)
                    .map_err(|_| Error::Invalid)?
                    .into(),
            ),
            8 => Value::Bytes(self.blob()?.to_vec()),
            9 => {
                let n = self.length()?;
                if n > self.nodes || n > self.bytes.len() {
                    return Err(Error::Limit);
                }
                let mut fields = Vec::new();
                for _ in 0..n {
                    let name = std::str::from_utf8(self.blob()?)
                        .map_err(|_| Error::Invalid)?
                        .into();
                    fields.push((name, self.value(depth + 1)?));
                }
                Value::Object(fields)
            }
            10 => {
                let n = self.length()?;
                if n > self.nodes || n > self.bytes.len() {
                    return Err(Error::Limit);
                }
                let mut values = Vec::new();
                for _ in 0..n {
                    values.push(self.value(depth + 1)?);
                }
                Value::Array(values)
            }
            _ => return Err(Error::Unsupported),
        })
    }
}
