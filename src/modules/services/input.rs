//! Logical assessment metadata distinguishes known values from bounded unresolved byte/text references.
use super::{
    Error,
    value::{Budget, Value},
};
use std::collections::BTreeSet;
/// Platform-independent input description before complete body transfer; it carries no path or peer wire ID.
#[derive(Clone)]
pub enum Input {
    /// Complete logical content already owned locally.
    Complete(Value),
    /// Structured metadata, retaining exact known field names and nesting.
    Object(Vec<(String, Input)>),
    /// Ordered logical items.
    Array(Vec<Input>),
    /// Unresolved exact bytes; text additionally requires final UTF-8 validation.
    Body {
        /// Positive or empty logical byte length under the caller's resource policy.
        length: u64,
        /// True for logical UTF-8 string content.
        text: bool,
    },
}
impl Input {
    /// Check aggregate logical depth/nodes/declared bytes before invoking a handler or allocating body storage.
    /// # Errors
    /// Invalid rejects duplicate/empty object fields; Limit rejects aggregate capacity or address-space overflow.
    pub fn bounded(&self, mut budget: Budget) -> Result<usize, Error> {
        tracing::trace!("Validating bounded unresolved service input metadata");
        budget.depth = budget.depth.min(31);
        let maximum = budget.bytes;
        self.walk(0, &mut budget)?;
        Ok(maximum - budget.bytes)
    }
    /// Count unresolved references and complete logical byte buffers under the same structural budgets.
    /// # Errors
    /// Returns the same malformed/budget errors as bounded and rejects arithmetic overflow.
    pub fn body_count(&self, budget: Budget) -> Result<usize, Error> {
        tracing::trace!("Counting bounded assessment body references");
        self.bounded(budget)?;
        self.count_bodies(budget)
    }
    // Count only previously bounded nodes; object/array traversal cannot exceed the admitted tree.
    fn count_bodies(&self, budget: Budget) -> Result<usize, Error> {
        match self {
            Self::Complete(value) => value.body_count(budget),
            Self::Body { .. } => Ok(1),
            Self::Object(fields) => fields.iter().try_fold(0usize, |n, (_, v)| {
                n.checked_add(v.count_bodies(budget)?).ok_or(Error::Limit)
            }),
            Self::Array(values) => values.iter().try_fold(0usize, |n, v| {
                n.checked_add(v.count_bodies(budget)?).ok_or(Error::Limit)
            }),
        }
    }
    /// Obtain a complete logical input only when no unresolved content remains.
    /// # Errors
    /// Any body reference returns Unsupported; callers must never replace it with placeholder payload bytes.
    pub fn complete(&self, budget: Budget) -> Result<Value, Error> {
        tracing::trace!("Resolving only already complete service assessment metadata");
        self.bounded(budget)?;
        self.materialize()
    }
    // Build the already bounded logical tree without unresolved bytes or repeated per-node logging.
    fn materialize(&self) -> Result<Value, Error> {
        match self {
            Self::Complete(value) => Ok(value.clone()),
            Self::Object(fields) => Ok(Value::Object(
                fields
                    .iter()
                    .map(|(n, v)| Ok((n.clone(), v.materialize()?)))
                    .collect::<Result<_, Error>>()?,
            )),
            Self::Array(items) => Ok(Value::Array(
                items
                    .iter()
                    .map(Self::materialize)
                    .collect::<Result<_, _>>()?,
            )),
            Self::Body { .. } => Err(Error::Unsupported),
        }
    }
    // Charge each logical node once; complete subtrees retain the same depth and aggregate accounting.
    fn walk(&self, depth: usize, budget: &mut Budget) -> Result<(), Error> {
        if let Self::Complete(value) = self {
            return walk_value(value, depth, budget);
        }
        node(depth, budget)?;
        match self {
            Self::Body { length, .. } => {
                charge(budget, usize::try_from(*length).map_err(|_| Error::Limit)?)
            }
            Self::Object(fields) => {
                let mut names = BTreeSet::new();
                for (name, value) in fields {
                    if name.is_empty() || !names.insert(name) {
                        return Err(Error::Invalid);
                    }
                    charge(budget, name.len())?;
                    value.walk(depth + 1, budget)?;
                }
                Ok(())
            }
            Self::Array(items) => {
                for value in items {
                    value.walk(depth + 1, budget)?;
                }
                Ok(())
            }
            Self::Complete(_) => Err(Error::Invalid),
        }
    }
}
// Traverse complete Value children without cloning or invoking unrelated module-private state.
fn walk_value(value: &Value, depth: usize, budget: &mut Budget) -> Result<(), Error> {
    node(depth, budget)?;
    match value {
        Value::String(s) => charge(budget, s.len()),
        Value::Bytes(b) => charge(budget, b.len()),
        Value::Object(fields) => {
            let mut names = BTreeSet::new();
            for (name, value) in fields {
                if name.is_empty() || !names.insert(name) {
                    return Err(Error::Invalid);
                }
                charge(budget, name.len())?;
                walk_value(value, depth + 1, budget)?;
            }
            Ok(())
        }
        Value::Array(items) => {
            for value in items {
                walk_value(value, depth + 1, budget)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
// Bound recursive nodes/depth before visiting descendants, omitting per-node logs for attacker-controlled trees.
fn node(depth: usize, budget: &mut Budget) -> Result<(), Error> {
    if depth > budget.depth {
        return Err(Error::Limit);
    }
    budget.nodes = budget.nodes.checked_sub(1).ok_or(Error::Limit)?;
    Ok(())
}
// Declared body lengths consume the same logical byte budget as owned complete byte/text content.
fn charge(budget: &mut Budget, length: usize) -> Result<(), Error> {
    budget.bytes = budget.bytes.checked_sub(length).ok_or(Error::Limit)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    // Metadata can describe large content without allocating it, but cannot masquerade as complete or exceed finite admission.
    #[test]
    fn unresolved_bounds_and_no_placeholders() {
        let b = Budget {
            depth: 31,
            nodes: 8,
            bytes: 100,
        };
        let input = Input::Object(vec![(
            "body".into(),
            Input::Body {
                length: 96,
                text: false,
            },
        )]);
        assert_eq!(input.bounded(b), Ok(100));
        assert_eq!(input.complete(b), Err(Error::Unsupported));
        assert_eq!(input.bounded(Budget { bytes: 99, ..b }), Err(Error::Limit));
        let too_large = Input::Body {
            length: u64::MAX,
            text: true,
        };
        assert_eq!(too_large.bounded(b), Err(Error::Limit));
        let duplicate = Input::Object(vec![
            ("x".into(), Input::Complete(Value::Bool(true))),
            ("x".into(), Input::Complete(Value::Bool(false))),
        ]);
        assert_eq!(duplicate.bounded(b), Err(Error::Invalid));
        let complete = Input::Array(vec![
            Input::Complete(Value::String("€".into())),
            Input::Complete(Value::Bytes(vec![0, 255])),
        ]);
        assert_eq!(complete.bounded(b), Ok(5));
        assert_eq!(
            complete.complete(b),
            Ok(Value::Array(vec![
                Value::String("€".into()),
                Value::Bytes(vec![0, 255])
            ]))
        );
        assert_eq!(
            complete.bounded(Budget { nodes: 1, ..b }),
            Err(Error::Limit)
        );
    }
}
