//! Lossless local values consumed by Services validators; peer codecs belong to Peer Protocol.
use super::Error;

/// Logical values preserve integer widths and floating-point bits without JSON coercion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    /// Logical boolean.
    Bool(bool),
    /// Signed 32-bit integer.
    Int32(i32),
    /// Signed 64-bit integer.
    Int64(i64),
    /// Unsigned 32-bit integer.
    Uint32(u32),
    /// Unsigned 64-bit integer.
    Uint64(u64),
    /// IEEE-754 binary32 bits, including signed zero and NaN payloads.
    Float(u32),
    /// IEEE-754 binary64 bits.
    Double(u64),
    /// Valid UTF-8 without normalization.
    String(String),
    /// Arbitrary bytes, including empty content.
    Bytes(Vec<u8>),
    /// Named fields; a sequence permits detecting duplicate names before interpretation.
    Object(Vec<(String, Value)>),
    /// Ordered logical values.
    Array(Vec<Value>),
}
/// Finite validation budgets, applied independently of schema shape.
#[derive(Clone, Copy)]
pub struct Budget {
    /// Maximum nested container depth, with root at zero.
    pub depth: usize,
    /// Maximum aggregate value nodes.
    pub nodes: usize,
    /// Maximum aggregate string, byte and field-name bytes.
    pub bytes: usize,
}
impl Value {
    /// Check aggregate bounds and duplicate object fields before schema interpretation.
    /// # Errors
    /// Limit denotes exhausted budgets; Invalid denotes duplicate or empty field names.
    pub fn bounded(&self, mut budget: Budget) -> Result<(), Error> {
        tracing::trace!("Checking logical value admission budgets");
        budget.depth = budget.depth.min(31);
        self.walk(0, &mut budget)
    }
    /// Measure aggregate text, byte and field-name bytes under independent structural budgets.
    /// # Errors
    /// Returns the same malformed-object and admission failures as bounded.
    pub fn byte_len(&self, mut budget: Budget) -> Result<usize, Error> {
        tracing::trace!("Measuring bounded logical input bytes for caller authority");
        budget.depth = budget.depth.min(31);
        let maximum = budget.bytes;
        self.walk(0, &mut budget)?;
        Ok(maximum - budget.bytes)
    }
    // Walk only bounded nesting and decrement shared aggregate budgets before visiting children.
    fn walk(&self, depth: usize, budget: &mut Budget) -> Result<(), Error> {
        if depth > budget.depth {
            return Err(Error::Limit);
        }
        budget.nodes = budget.nodes.checked_sub(1).ok_or(Error::Limit)?;
        match self {
            Self::String(s) => consume(budget, s.len())?,
            Self::Bytes(b) => consume(budget, b.len())?,
            Self::Object(fields) => {
                let mut names = std::collections::BTreeSet::new();
                for (name, value) in fields {
                    consume(budget, name.len())?;
                    if name.is_empty() || !names.insert(name) {
                        return Err(Error::Invalid);
                    }
                    value.walk(depth + 1, budget)?;
                }
            }
            Self::Array(values) => {
                for value in values {
                    value.walk(depth + 1, budget)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}
// Charge text/byte storage with checked subtraction, including ignored unknown fields.
fn consume(budget: &mut Budget, count: usize) -> Result<(), Error> {
    budget.bytes = budget.bytes.checked_sub(count).ok_or(Error::Limit)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    // Literal extrema and float bits survive ownership without floating-point integer conversion.
    #[test]
    fn exact_values() {
        let values = Value::Array(vec![
            Value::Uint64(u64::MAX),
            Value::Int64(i64::MIN),
            Value::Int32(i32::MAX),
            Value::Uint32(u32::MAX),
            Value::Float(0x80000000),
            Value::Double(0x7ff8000000000001),
            Value::Bool(true),
            Value::String("€".into()),
            Value::Bytes(vec![0, 255]),
        ]);
        assert_eq!(
            values.bounded(Budget {
                depth: 1,
                nodes: 10,
                bytes: 5
            }),
            Ok(())
        );
        assert_eq!(values.clone(), values);
        assert_ne!(Value::Float(0), Value::Float(0x80000000));
    }
    // Depth, aggregate nodes/bytes and duplicate fields fail independently before interpretation.
    #[test]
    fn admission_failures() {
        let v = Value::Array(vec![Value::Bytes(vec![1])]);
        for b in [
            Budget {
                depth: 0,
                nodes: 2,
                bytes: 1,
            },
            Budget {
                depth: 1,
                nodes: 1,
                bytes: 1,
            },
            Budget {
                depth: 1,
                nodes: 2,
                bytes: 0,
            },
        ] {
            assert_eq!(v.bounded(b), Err(Error::Limit));
        }
        let b = Budget {
            depth: 2,
            nodes: 8,
            bytes: 10,
        };
        for fields in [
            vec![("".into(), Value::Bool(false))],
            vec![
                ("x".into(), Value::Bool(false)),
                ("x".into(), Value::Bool(true)),
            ],
        ] {
            assert_eq!(Value::Object(fields).bounded(b), Err(Error::Invalid));
        }
        assert_eq!(
            Value::Object(vec![("x".into(), Value::Array(vec![]))]).bounded(b),
            Ok(())
        );
    }
}
