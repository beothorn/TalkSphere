//! Operation-specific logical value checks, layered after bounded Services schema validation.
use crate::modules::services::{
    Error,
    value::{Budget, Value},
};

/// The ordinary invocation discriminant; remote byte checks use their separate schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    /// Create a new immutable lease.
    Store,
    /// Read an existing lease range.
    Retrieve,
    /// Extend an existing active lease.
    Renew,
    /// End an existing active lease.
    Remove,
}
const INPUT_FIELDS: &[&str] = &[
    "operation",
    "leaseId",
    "sizeBytes",
    "retentionSeconds",
    "contentHash",
    "data",
    "offset",
    "length",
    "expectedExpiresAt",
    "additionalSeconds",
];
#[cfg(test)]
const OUTPUT_FIELDS: &[&str] = &[
    "operation",
    "leaseId",
    "sizeBytes",
    "contentHash",
    "acceptedAt",
    "previousExpiresAt",
    "expiresAt",
    "offset",
    "data",
    "price",
    "removed",
];

/// Validate complete invocation input shape without authorizing work or consulting mutable lease state.
/// Unknown extension fields remain bounded inert values.
/// # Errors
/// Invalid rejects missing/forbidden known fields, wrong widths/lengths and zero required quantities.
/// Limit rejects exhausted aggregate structural/byte budgets before semantic inspection.
pub fn input(value: &Value, budget: Budget) -> Result<Operation, Error> {
    tracing::trace!("Validating operation-specific Storage input fields");
    value.bounded(budget)?;
    let fields = fields(value)?;
    let operation = operation(fields)?;
    let required = match operation {
        Operation::Store => &[
            "operation",
            "sizeBytes",
            "retentionSeconds",
            "contentHash",
            "data",
        ][..],
        Operation::Retrieve => &["operation", "leaseId", "offset", "length"][..],
        Operation::Renew => &[
            "operation",
            "leaseId",
            "expectedExpiresAt",
            "additionalSeconds",
        ][..],
        Operation::Remove => &["operation", "leaseId"][..],
    };
    selected(fields, required, INPUT_FIELDS)?;
    match operation {
        Operation::Store => {
            positive(fields, "sizeBytes")?;
            positive(fields, "retentionSeconds")?;
            fixed(fields, "contentHash", 32)?;
            bytes(fields, "data")?;
        }
        Operation::Retrieve => {
            fixed(fields, "leaseId", 32)?;
            number(fields, "offset")?;
            positive(fields, "length")?;
        }
        Operation::Renew => {
            fixed(fields, "leaseId", 32)?;
            number(fields, "expectedExpiresAt")?;
            positive(fields, "additionalSeconds")?;
        }
        Operation::Remove => {
            fixed(fields, "leaseId", 32)?;
        }
    }
    Ok(operation)
}

/// Validate successful output shape, leaving original-request comparison to requester verification.
/// # Errors
/// Invalid rejects wrong discriminants, missing/forbidden fields and malformed success values.
/// Limit denotes exhausted independent value budgets.
pub fn output(value: &Value, expected: Operation, budget: Budget) -> Result<(), Error> {
    tracing::trace!("Validating operation-specific Storage successful output");
    value.bounded(budget)?;
    let f = fields(value)?;
    if operation(f)? != expected {
        return Err(Error::Invalid);
    }
    let required = match expected {
        Operation::Store => &[
            "operation",
            "leaseId",
            "sizeBytes",
            "contentHash",
            "acceptedAt",
            "expiresAt",
            "price",
        ][..],
        Operation::Retrieve => &[
            "operation",
            "leaseId",
            "offset",
            "data",
            "contentHash",
            "price",
        ][..],
        Operation::Renew => &[
            "operation",
            "leaseId",
            "previousExpiresAt",
            "expiresAt",
            "price",
        ][..],
        Operation::Remove => &["operation", "leaseId", "removed", "price"][..],
    };
    selected(f, required, &[])?;
    fixed(f, "leaseId", 32)?;
    positive(f, "price")?;
    match expected {
        Operation::Store => {
            positive(f, "sizeBytes")?;
            fixed(f, "contentHash", 32)?;
            number(f, "acceptedAt")?;
            number(f, "expiresAt")?;
        }
        Operation::Retrieve => {
            number(f, "offset")?;
            fixed(f, "contentHash", 32)?;
            if bytes(f, "data")?.is_empty() {
                return Err(Error::Invalid);
            }
        }
        Operation::Renew => {
            number(f, "previousExpiresAt")?;
            number(f, "expiresAt")?;
        }
        Operation::Remove => {
            if get(f, "removed")? != &Value::Bool(true) {
                return Err(Error::Invalid);
            }
        }
    }
    Ok(())
}

/// Validate a public byte challenge independently of ordinary invocation discriminants.
/// # Errors
/// Invalid rejects malformed IDs, offsets and known private/outcome fields; Limit rejects excessive values.
pub fn challenge(value: &Value, budget: Budget) -> Result<(), Error> {
    tracing::trace!("Validating public Storage byte challenge");
    value.bounded(budget)?;
    let f = fields(value)?;
    fixed(f, "leaseId", 32)?;
    number(f, "offset")?;
    if f.iter().any(|(name, _)| {
        INPUT_FIELDS
            .iter()
            .any(|known| name == known && !matches!(*known, "leaseId" | "offset"))
            || matches!(
                name.as_str(),
                "value" | "outcome" | "expected" | "expectedByte"
            )
    }) {
        return Err(Error::Invalid);
    }
    Ok(())
}

/// Validate remote evidence shape without accepting a provider-computed correctness conclusion.
/// # Errors
/// Invalid rejects missing/malformed fields or a provider outcome; Limit rejects budget exhaustion.
pub fn evidence(value: &Value, budget: Budget) -> Result<(), Error> {
    tracing::trace!("Validating Storage byte evidence");
    value.bounded(budget)?;
    let f = fields(value)?;
    fixed(f, "leaseId", 32)?;
    number(f, "offset")?;
    fixed(f, "value", 1)?;
    if f.iter().any(|(name, _)| name == "outcome") {
        return Err(Error::Invalid);
    }
    Ok(())
}

// Require an object after aggregate validation has rejected duplicate names.
fn fields(value: &Value) -> Result<&[(String, Value)], Error> {
    tracing::trace!("Reading Storage operation object");
    if let Value::Object(f) = value {
        Ok(f)
    } else {
        Err(Error::Invalid)
    }
}
// Interpret only the standard invocation discriminator, never arbitrary extension text.
fn operation(f: &[(String, Value)]) -> Result<Operation, Error> {
    tracing::trace!("Reading Storage operation discriminator");
    match get(f, "operation")? {
        Value::String(s) => match s.as_str() {
            "store" => Ok(Operation::Store),
            "retrieve" => Ok(Operation::Retrieve),
            "renew" => Ok(Operation::Renew),
            "remove" => Ok(Operation::Remove),
            _ => Err(Error::Invalid),
        },
        _ => Err(Error::Invalid),
    }
}
// All selected fields are required; other known-operation fields are forbidden.
fn selected(f: &[(String, Value)], required: &[&str], known: &[&str]) -> Result<(), Error> {
    tracing::trace!("Checking selected Storage operation fields");
    for name in required {
        get(f, name)?;
    }
    if f.iter()
        .any(|(n, _)| known.contains(&n.as_str()) && !required.contains(&n.as_str()))
    {
        return Err(Error::Invalid);
    }
    Ok(())
}
// Borrow an exact field without normalizing absent values or creating placeholders.
fn get<'a>(f: &'a [(String, Value)], name: &str) -> Result<&'a Value, Error> {
    tracing::trace!("Reading required Storage field");
    f.iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v)
        .ok_or(Error::Invalid)
}
// Numeric values must retain the exact declared Services uint64 kind.
fn number(f: &[(String, Value)], name: &str) -> Result<u64, Error> {
    tracing::trace!("Checking exact Storage integer field");
    if let Value::Uint64(n) = get(f, name)? {
        Ok(*n)
    } else {
        Err(Error::Invalid)
    }
}
// Positive sizes/prices/durations cannot be zero even when represented with the correct type.
fn positive(f: &[(String, Value)], name: &str) -> Result<u64, Error> {
    tracing::trace!("Checking positive Storage integer field");
    let n = number(f, name)?;
    if n == 0 { Err(Error::Invalid) } else { Ok(n) }
}
// Complete bytes differ from unresolved metadata and are borrowed without an extra content copy.
fn bytes<'a>(f: &'a [(String, Value)], name: &str) -> Result<&'a [u8], Error> {
    tracing::trace!("Checking complete Storage byte field");
    if let Value::Bytes(b) = get(f, name)? {
        Ok(b)
    } else {
        Err(Error::Invalid)
    }
}
// Fixed identifiers and hashes must contain the specified exact number of bytes.
fn fixed(f: &[(String, Value)], name: &str, length: usize) -> Result<(), Error> {
    tracing::trace!("Checking fixed Storage byte width");
    if bytes(f, name)?.len() == length {
        Ok(())
    } else {
        Err(Error::Invalid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const B: Budget = Budget {
        depth: 5,
        nodes: 100,
        bytes: 1000,
    };
    // Build complete logical scenarios directly, independently of any wire encoder.
    fn object(fields: Vec<(&str, Value)>) -> Value {
        Value::Object(fields.into_iter().map(|(n, v)| (n.into(), v)).collect())
    }
    // Every standard operation has a valid success path and uint64 fields retain exact extrema.
    fn inputs() -> Vec<(Operation, Value)> {
        vec![
            (
                Operation::Store,
                object(vec![
                    ("operation", Value::String("store".into())),
                    ("sizeBytes", Value::Uint64(3)),
                    ("retentionSeconds", Value::Uint64(10)),
                    ("contentHash", Value::Bytes(vec![0; 32])),
                    ("data", Value::Bytes(b"abc".to_vec())),
                ]),
            ),
            (
                Operation::Retrieve,
                object(vec![
                    ("operation", Value::String("retrieve".into())),
                    ("leaseId", Value::Bytes(vec![1; 32])),
                    ("offset", Value::Uint64(u64::MAX)),
                    ("length", Value::Uint64(1)),
                ]),
            ),
            (
                Operation::Renew,
                object(vec![
                    ("operation", Value::String("renew".into())),
                    ("leaseId", Value::Bytes(vec![1; 32])),
                    ("expectedExpiresAt", Value::Uint64(u64::MAX)),
                    ("additionalSeconds", Value::Uint64(1)),
                ]),
            ),
            (
                Operation::Remove,
                object(vec![
                    ("operation", Value::String("remove".into())),
                    ("leaseId", Value::Bytes(vec![1; 32])),
                ]),
            ),
        ]
    }
    // Missing, duplicate, wrong-kind and cross-operation fields fail while unknown extensions remain inert.
    #[test]
    fn input_validation_matrix() {
        for (op, value) in inputs() {
            assert_eq!(input(&value, B), Ok(op));
            let Value::Object(f) = value else {
                unreachable!()
            };
            for index in 0..f.len() {
                let mut bad = f.clone();
                bad.remove(index);
                assert_eq!(input(&Value::Object(bad), B), Err(Error::Invalid));
                let mut bad = f.clone();
                bad[index].1 = Value::Bool(false);
                assert_eq!(input(&Value::Object(bad), B), Err(Error::Invalid));
            }
            let mut extra = f.clone();
            extra.push(("future".into(), Value::Bool(true)));
            assert_eq!(input(&Value::Object(extra), B), Ok(op));
            for known in INPUT_FIELDS {
                if !f.iter().any(|(n, _)| n == known) {
                    let mut bad = f.clone();
                    bad.push((known.to_string(), Value::Bool(true)));
                    assert_eq!(input(&Value::Object(bad), B), Err(Error::Invalid));
                }
            }
            let mut duplicate = f.clone();
            duplicate.push(f[0].clone());
            assert_eq!(input(&Value::Object(duplicate), B), Err(Error::Invalid));
        }
        assert_eq!(input(&Value::Bool(true), B), Err(Error::Invalid));
        assert_eq!(
            input(&inputs()[0].1, Budget { nodes: 1, ..B }),
            Err(Error::Limit)
        );
        let mut bad = inputs()[0].1.clone();
        if let Value::Object(f) = &mut bad {
            f[1].1 = Value::Uint64(0);
        }
        assert_eq!(input(&bad, B), Err(Error::Invalid));
    }
    // Successful output fields are mandatory even when the shared union-like schema marks them optional.
    #[test]
    fn output_validation_matrix() {
        for (op, _) in inputs() {
            let name = match op {
                Operation::Store => "store",
                Operation::Retrieve => "retrieve",
                Operation::Renew => "renew",
                Operation::Remove => "remove",
            };
            let mut f = vec![
                ("operation", Value::String(name.into())),
                ("leaseId", Value::Bytes(vec![1; 32])),
                ("price", Value::Uint64(1)),
            ];
            match op {
                Operation::Store => f.extend([
                    ("sizeBytes", Value::Uint64(3)),
                    ("contentHash", Value::Bytes(vec![0; 32])),
                    ("acceptedAt", Value::Uint64(1000)),
                    ("expiresAt", Value::Uint64(1010)),
                ]),
                Operation::Retrieve => f.extend([
                    ("offset", Value::Uint64(1)),
                    ("data", Value::Bytes(vec![2])),
                    ("contentHash", Value::Bytes(vec![0; 32])),
                ]),
                Operation::Renew => f.extend([
                    ("previousExpiresAt", Value::Uint64(1000)),
                    ("expiresAt", Value::Uint64(1010)),
                ]),
                Operation::Remove => f.push(("removed", Value::Bool(true))),
            }
            let good = object(f.clone());
            assert_eq!(output(&good, op, B), Ok(()));
            for index in 0..f.len() {
                let mut bad = f.clone();
                bad.remove(index);
                assert_eq!(output(&object(bad), op, B), Err(Error::Invalid));
                let mut bad = f.clone();
                bad[index].1 = Value::Bool(false);
                assert_eq!(output(&object(bad), op, B), Err(Error::Invalid));
            }
            assert!(OUTPUT_FIELDS.contains(&"price"));
        }
    }
    // Public evidence has one byte; the expected byte and correctness conclusion stay local.
    #[test]
    fn challenge_and_evidence_boundaries() {
        let f = vec![
            ("leaseId", Value::Bytes(vec![1; 32])),
            ("offset", Value::Uint64(u64::MAX)),
        ];
        assert_eq!(challenge(&object(f.clone()), B), Ok(()));
        for forbidden in ["operation", "value", "outcome", "expected", "expectedByte"] {
            let mut bad = f.clone();
            bad.push((forbidden, Value::Bool(false)));
            assert_eq!(challenge(&object(bad), B), Err(Error::Invalid));
        }
        let mut e = f.clone();
        e.push(("value", Value::Bytes(vec![4])));
        assert_eq!(evidence(&object(e.clone()), B), Ok(()));
        e[2].1 = Value::Bytes(vec![]);
        assert_eq!(evidence(&object(e.clone()), B), Err(Error::Invalid));
        e[2].1 = Value::Bytes(vec![4]);
        e.push(("outcome", Value::String("pass".into())));
        assert_eq!(evidence(&object(e), B), Err(Error::Invalid));
        let mut short = f;
        short[0].1 = Value::Bytes(vec![0; 31]);
        assert_eq!(challenge(&object(short), B), Err(Error::Invalid));
    }
}
