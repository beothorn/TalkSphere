//! The Page check: local verification of one successful `get` result.
//!
//! Callers: the requester client and the viewer. The check sends nothing to the publisher and costs nothing: it is included in each successful invocation. The
//! visitor computes the outcome itself and never trusts a publisher's claim that a result passed. A pass establishes only that the bytes and metadata are
//! self-consistent with the request and the agreed price; it proves nothing about truth, safety or authorship and creates no transferable proof.
use crate::modules::{cryptography::primitives::hash, pages::media::MediaType};

/// The visitor-computed outcome of a check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The path, media type, size, hash and price are all consistent with the request and the agreement.
    Pass,
    /// Every needed value is available but at least one pass condition is false.
    Fail,
    /// A needed input is unavailable or a local limit prevented the complete check.
    Inconclusive,
}

impl Outcome {
    /// The logical name: `pass`, `fail` or `inconclusive`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Inconclusive => "inconclusive",
        }
    }
}

/// A returned output as the visitor received it, kept as raw values so that an invalid label, path or hash is a failure rather than an unavailable input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Returned {
    /// The returned path text.
    pub path: String,
    /// The returned media type text.
    pub media_type: String,
    /// The declared content size.
    pub content_size: u64,
    /// The declared content hash bytes.
    pub content_hash: Vec<u8>,
    /// The complete content bytes.
    pub content: Vec<u8>,
    /// The returned price.
    pub price: u64,
}

/// What the visitor holds about one `get`: its own request, the price it agreed to and the output it received. `None` means the value is unavailable.
#[derive(Debug, Clone, Default)]
pub struct Evidence {
    /// The canonical path text the visitor requested.
    pub requested_path: Option<String>,
    /// The price the visitor agreed to before the invocation.
    pub agreed_price: Option<u64>,
    /// The complete output that was returned.
    pub returned: Option<Returned>,
}

/// Judge one result. Missing inputs are inconclusive; otherwise any false condition fails and all true passes.
pub fn evaluate(evidence: &Evidence) -> Outcome {
    tracing::trace!("Judging a Page get result locally");
    let (Some(requested), Some(agreed), Some(returned)) = (
        evidence.requested_path.as_ref(),
        evidence.agreed_price,
        evidence.returned.as_ref(),
    ) else {
        return Outcome::Inconclusive;
    };
    let Ok(actual) = hash::sha256(&returned.content, returned.content.len()) else {
        // The content is above the local hashing budget: no complete check is possible.
        return Outcome::Inconclusive;
    };
    let pass = returned.path == *requested
        && MediaType::parse(&returned.media_type).is_ok()
        && returned.content_size == returned.content.len() as u64
        && returned.content_hash == actual
        && returned.price == agreed;
    if pass { Outcome::Pass } else { Outcome::Fail }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The PG-02 result: root, text/html, `abc`, its SHA-256 and price 4.
    fn result() -> Evidence {
        let content = b"abc".to_vec();
        Evidence {
            requested_path: Some("/".into()),
            agreed_price: Some(4),
            returned: Some(Returned {
                path: "/".into(),
                media_type: "text/html".into(),
                content_size: 3,
                content_hash: hash::sha256(&content, 3).unwrap().to_vec(),
                content,
                price: 4,
            }),
        }
    }

    // PG-09: the complete PG-02 result passes, with the known SHA-256 of `abc`.
    #[test]
    fn the_honest_result_passes() {
        let evidence = result();
        assert_eq!(evaluate(&evidence), Outcome::Pass);
        let hash: String = evidence
            .returned
            .unwrap()
            .content_hash
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(
            hash,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(Outcome::Pass.name(), "pass");
    }

    // PG-09: a different path, an invalid media type, a wrong size, a hash of the wrong value or length, a different price and modified content
    // each fail.
    #[test]
    fn any_false_condition_fails() {
        let edits: Vec<fn(&mut Evidence)> = vec![
            |e| e.returned.as_mut().unwrap().path = "/other".into(),
            |e| e.returned.as_mut().unwrap().media_type = "Text/HTML".into(),
            |e| e.returned.as_mut().unwrap().content_size = 4,
            |e| e.returned.as_mut().unwrap().content_hash[0] ^= 1,
            |e| {
                e.returned
                    .as_mut()
                    .unwrap()
                    .content_hash
                    .pop()
                    .map(drop)
                    .unwrap_or(())
            },
            |e| e.agreed_price = Some(5),
            |e| e.returned.as_mut().unwrap().price = 3,
            |e| e.returned.as_mut().unwrap().content[0] ^= 1,
        ];
        for edit in edits {
            let mut evidence = result();
            edit(&mut evidence);
            assert_eq!(evaluate(&evidence), Outcome::Fail);
        }
    }

    // PG-09: a missing request, agreed price or output is inconclusive and never a pass.
    #[test]
    fn missing_inputs_are_inconclusive() {
        let clear: Vec<fn(&mut Evidence)> = vec![
            |e| e.requested_path = None,
            |e| e.agreed_price = None,
            |e| e.returned = None,
        ];
        for edit in clear {
            let mut evidence = result();
            edit(&mut evidence);
            assert_eq!(evaluate(&evidence), Outcome::Inconclusive);
        }
        assert_eq!(evaluate(&Evidence::default()), Outcome::Inconclusive);
        assert_eq!(Outcome::Fail.name(), "fail");
        assert_eq!(Outcome::Inconclusive.name(), "inconclusive");
    }
}
