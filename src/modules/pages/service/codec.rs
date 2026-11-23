//! Typed `get` requests and outputs of the Page service and their logical values.
//!
//! Callers: the provider adapter, the requester client and the viewer. The request is `{path, maximumBytes}` with no `operation` field; the output is
//! `{path, mediaType, contentSize, contentHash, content, price}`. Fields have exact logical types and lengths, an unknown field is ignored (compatible
//! extension) and duplicates and wrong types are invalid. The content length must equal the declared size. Wire encoding belongs to Peer Protocol.
use crate::modules::{
    pages::{error::Error, media::MediaType, path::CanonicalPath},
    services::value::Value,
};

/// A decoded `get` request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetRequest {
    /// The canonical path of the wanted resource.
    pub path: CanonicalPath,
    /// The largest content size the visitor accepts.
    pub maximum_bytes: u64,
}

/// A decoded `get` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetOutput {
    /// The returned path, byte for byte the requested path.
    pub path: CanonicalPath,
    /// The returned media type.
    pub media_type: MediaType,
    /// The exact content size in bytes.
    pub content_size: u64,
    /// The 32-byte SHA-256 of the content.
    pub content_hash: [u8; 32],
    /// The complete content bytes.
    pub content: Vec<u8>,
    /// The credits charged.
    pub price: u64,
}

/// Decode a request value.
/// # Errors
/// `Invalid` for a non-object, duplicate names, a missing or wrongly typed field, a path that is not canonical or a zero `maximumBytes`.
pub fn parse_request(value: &Value) -> Result<GetRequest, Error> {
    tracing::trace!("Decoding a Page get request");
    let fields = Fields::new(value)?;
    Ok(GetRequest {
        path: CanonicalPath::parse(fields.text("path")?.as_bytes())?,
        maximum_bytes: fields.u64("maximumBytes")?,
    })
}

/// Encode a request as the logical value a visitor sends.
pub fn request_value(request: &GetRequest) -> Value {
    tracing::trace!("Encoding a Page get request");
    Value::Object(vec![
        ("path".into(), Value::String(request.path.as_str().into())),
        ("maximumBytes".into(), Value::Uint64(request.maximum_bytes)),
    ])
}

/// Decode an output value for the visitor, checking every field's type and the content length.
/// # Errors
/// `Invalid` for a malformed value, a hash that is not 32 bytes or content whose length differs from `contentSize`.
pub fn parse_output(value: &Value) -> Result<GetOutput, Error> {
    tracing::trace!("Decoding a Page get output");
    let fields = Fields::new(value)?;
    let content = fields.bytes("content")?.to_vec();
    let content_size = fields.u64("contentSize")?;
    if content.len() as u64 != content_size {
        return Err(Error::Invalid);
    }
    Ok(GetOutput {
        path: CanonicalPath::parse(fields.text("path")?.as_bytes())?,
        media_type: MediaType::parse(fields.text("mediaType")?)?,
        content_size,
        content_hash: fields
            .bytes("contentHash")?
            .try_into()
            .map_err(|_| Error::Invalid)?,
        content,
        price: fields.u64("price")?,
    })
}

/// Encode an output as the logical value a provider returns.
pub fn output_value(output: &GetOutput) -> Value {
    tracing::trace!("Encoding a Page get output");
    Value::Object(vec![
        ("path".into(), Value::String(output.path.as_str().into())),
        (
            "mediaType".into(),
            Value::String(output.media_type.as_str().into()),
        ),
        ("contentSize".into(), Value::Uint64(output.content_size)),
        (
            "contentHash".into(),
            Value::Bytes(output.content_hash.to_vec()),
        ),
        ("content".into(), Value::Bytes(output.content.clone())),
        ("price".into(), Value::Uint64(output.price)),
    ])
}

// Typed read access to one object's fields, rejecting duplicates up front.
struct Fields<'a>(&'a [(String, Value)]);
impl<'a> Fields<'a> {
    fn new(value: &'a Value) -> Result<Self, Error> {
        let Value::Object(fields) = value else {
            return Err(Error::Invalid);
        };
        for (index, (name, _)) in fields.iter().enumerate() {
            if fields[..index].iter().any(|(other, _)| other == name) {
                return Err(Error::Invalid);
            }
        }
        Ok(Self(fields))
    }
    fn get(&self, name: &str) -> Option<&'a Value> {
        self.0.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }
    fn text(&self, name: &str) -> Result<&'a str, Error> {
        match self.get(name) {
            Some(Value::String(s)) => Ok(s),
            _ => Err(Error::Invalid),
        }
    }
    fn bytes(&self, name: &str) -> Result<&'a [u8], Error> {
        match self.get(name) {
            Some(Value::Bytes(b)) => Ok(b),
            _ => Err(Error::Invalid),
        }
    }
    fn u64(&self, name: &str) -> Result<u64, Error> {
        match self.get(name) {
            Some(Value::Uint64(n)) => Ok(*n),
            _ => Err(Error::Invalid),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output() -> GetOutput {
        GetOutput {
            path: CanonicalPath::root(),
            media_type: MediaType::parse("text/html").unwrap(),
            content_size: 3,
            content_hash: [7; 32],
            content: b"abc".to_vec(),
            price: 4,
        }
    }

    // A request and an output survive encoding and decoding unchanged, and an unknown field is ignored.
    #[test]
    fn values_round_trip() {
        let request = GetRequest {
            path: CanonicalPath::parse(b"/a/b").unwrap(),
            maximum_bytes: 3,
        };
        assert_eq!(parse_request(&request_value(&request)), Ok(request.clone()));
        assert_eq!(parse_output(&output_value(&output())), Ok(output()));
        let Value::Object(mut fields) = request_value(&request) else {
            unreachable!()
        };
        fields.push(("future".into(), Value::Bool(true)));
        assert_eq!(parse_request(&Value::Object(fields)), Ok(request));
    }

    // PG-03/PG-05: a missing or mistyped member, a non-canonical path, a bad media type, a short hash, a length that differs from the size and
    // duplicate names are all invalid.
    #[test]
    fn malformed_values_are_rejected() {
        let good = request_value(&GetRequest {
            path: CanonicalPath::root(),
            maximum_bytes: 3,
        });
        let Value::Object(fields) = good.clone() else {
            unreachable!()
        };
        for name in ["path", "maximumBytes"] {
            let without: Vec<_> = fields.iter().filter(|(n, _)| n != name).cloned().collect();
            assert_eq!(parse_request(&Value::Object(without)), Err(Error::Invalid));
        }
        let with = |path: &str, bytes: Value| {
            Value::Object(vec![
                ("path".into(), Value::String(path.into())),
                ("maximumBytes".into(), bytes),
            ])
        };
        assert_eq!(
            parse_request(&with("about", Value::Uint64(1))),
            Err(Error::Invalid)
        );
        assert_eq!(
            parse_request(&with("/", Value::Uint32(1))),
            Err(Error::Invalid)
        );
        let duplicate = Value::Object(vec![
            fields[0].clone(),
            fields[0].clone(),
            fields[1].clone(),
        ]);
        assert_eq!(parse_request(&duplicate), Err(Error::Invalid));
        assert_eq!(parse_request(&Value::Bool(true)), Err(Error::Invalid));
        let Value::Object(out) = output_value(&output()) else {
            unreachable!()
        };
        let edit = |name: &str, value: Value| {
            Value::Object(
                out.iter()
                    .map(|(n, v)| {
                        if n == name {
                            (n.clone(), value.clone())
                        } else {
                            (n.clone(), v.clone())
                        }
                    })
                    .collect(),
            )
        };
        assert_eq!(
            parse_output(&edit("contentSize", Value::Uint64(4))),
            Err(Error::Invalid)
        );
        assert_eq!(
            parse_output(&edit("contentHash", Value::Bytes(vec![1; 31]))),
            Err(Error::Invalid)
        );
        assert_eq!(
            parse_output(&edit("mediaType", Value::String("Text/html".into()))),
            Err(Error::Invalid)
        );
        assert_eq!(
            parse_output(&edit("path", Value::String("/a/".into()))),
            Err(Error::Invalid)
        );
        assert_eq!(
            parse_output(&edit("price", Value::Bool(true))),
            Err(Error::Invalid)
        );
    }
}
