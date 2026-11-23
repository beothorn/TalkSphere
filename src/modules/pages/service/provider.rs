//! The Services provider adapter for the publisher-served Page.
//!
//! Callers: node composition (registration with the Services registry) and the coordinator. A `get` changes no state, so its effect is a read-only query: `execute`
//! validates the request against the selected offering, takes one immutable snapshot of the current resource and stages the complete result, keeping it in memory
//! so the bytes cannot change before the decision and journaling a durable record (the request and the offering terms it was staged under) so that a restart can
//! rebuild the staged result and a lost reply is answered from the original outcome. The coordinator then prepares, decides and commits as for any service; nothing
//! is charged for a failure. The publisher's identity is the provider; there is no remote check, only the visitor's local one.
use super::{
    codec::{self, GetOutput, GetRequest},
    store::Pages,
    terms::Terms,
};
use crate::modules::{
    board::offering::{Direction, Offering},
    cryptography::primitives::hash,
    hosted_publications::{
        effect::{Effect, Outcome},
        journal::{Entry, Journal},
    },
    identity::api::IdentityId,
    messaging::ports::Entropy,
    pages::path::CanonicalPath,
    services::{
        self, Error,
        checks::Invocation as CheckInvocation,
        contract::OperationKey,
        handler::{Assessment, Blob, Completion, Invocation, Peer, Status},
        value::{Budget, Value},
    },
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

/// The installed provider adapter for one publisher identity.
pub struct PageProvider {
    identity: IdentityId,
    pages: Arc<dyn Pages>,
    journal: Journal,
    entropy: Arc<dyn Entropy>,
    budget: Budget,
    maximum_bytes: usize,
    // Staged completions pinned in memory: the snapshot taken at staging is what is committed.
    staged: Mutex<BTreeMap<OperationKey, Completion>>,
}

impl PageProvider {
    /// Bind the publisher's identity, resource source and durable journal to the value bounds Services uses for results.
    /// # Errors
    /// `Invalid` for a zero bound.
    pub fn new(
        identity: IdentityId,
        pages: Arc<dyn Pages>,
        journal: Journal,
        entropy: Arc<dyn Entropy>,
        budget: Budget,
        maximum_bytes: usize,
    ) -> Result<Self, Error> {
        tracing::trace!("Binding the Page provider adapter");
        if budget.depth == 0 || budget.nodes == 0 || budget.bytes == 0 || maximum_bytes == 0 {
            return Err(Error::Invalid);
        }
        Ok(Self {
            identity,
            pages,
            journal,
            entropy,
            budget,
            maximum_bytes,
            staged: Mutex::new(BTreeMap::new()),
        })
    }

    // The caller's budget intersected with the installed one, so neither can enlarge the other.
    fn bounded(&self, caller: Budget) -> Budget {
        Budget {
            depth: caller.depth.min(self.budget.depth),
            nodes: caller.nodes.min(self.budget.nodes),
            bytes: caller.bytes.min(self.budget.bytes),
        }
    }

    // The request judged against the selected offering's limits: a canonical path within the path bound and a nonzero byte bound within the resource bound.
    fn request_for(&self, terms: &Terms, input: &Value) -> Result<GetRequest, Error> {
        let request = codec::parse_request(input).map_err(services::Error::from)?;
        if request.path.size() as u64 > terms.maximum_path_bytes
            || request.maximum_bytes == 0
            || request.maximum_bytes > terms.maximum_resource_bytes
        {
            return Err(Error::Invalid);
        }
        Ok(request)
    }

    // One snapshot of the current resource as a complete output, or the reason there is none. A missing resource is concealed like any unavailable one and is
    // never replaced by another resource or an error page.
    fn output_for(&self, terms: &Terms, request: &GetRequest) -> Result<GetOutput, Error> {
        let snapshot = self
            .pages
            .snapshot(&request.path)
            .map_err(services::Error::from)?
            .ok_or(Error::Denied)?;
        let size = snapshot.content.len() as u64;
        if size > request.maximum_bytes || size > terms.maximum_resource_bytes {
            return Err(Error::Limit);
        }
        let content_hash =
            hash::sha256(&snapshot.content, snapshot.content.len()).map_err(|_| Error::Limit)?;
        Ok(GetOutput {
            path: request.path.clone(),
            media_type: snapshot.media_type,
            content_size: size,
            content_hash,
            content: snapshot.content.as_ref().clone(),
            price: terms.price,
        })
    }

    // The staged completion of an output, carrying the exact durable effect bytes.
    fn completion_of(&self, output: &GetOutput, effect: &Effect) -> Result<Completion, Error> {
        let value = codec::output_value(output);
        let result = services::codec::encode(&value, self.budget, self.maximum_bytes)?;
        Ok(Completion {
            result: Blob::new(result, self.maximum_bytes)?,
            output: value,
            price: output.price,
            effect: Blob::new(effect.encode(), self.maximum_bytes)?,
        })
    }

    // The durable effect: the request and the terms it was staged under, so a restart can rebuild the staged result without the offering.
    fn effect_for(request: &GetRequest, terms: &Terms) -> Effect {
        let record = serde_json::json!({
            "path": request.path.as_str(),
            "maximumBytes": request.maximum_bytes,
            "price": terms.price,
            "maximumPathBytes": terms.maximum_path_bytes,
            "maximumResourceBytes": terms.maximum_resource_bytes,
        });
        Effect::Query {
            request: serde_json::to_vec(&record).unwrap_or_default(),
        }
    }

    // The request and terms a prepared effect carries.
    fn decode_effect(effect: &Effect) -> Result<(GetRequest, Terms), Error> {
        let Effect::Query { request } = effect else {
            return Err(Error::Invalid);
        };
        let value: serde_json::Value =
            serde_json::from_slice(request).map_err(|_| Error::Storage)?;
        let number = |name: &str| value[name].as_u64().ok_or(Error::Storage);
        let path = value["path"].as_str().ok_or(Error::Storage)?;
        Ok((
            GetRequest {
                path: CanonicalPath::parse(path.as_bytes()).map_err(|_| Error::Storage)?,
                maximum_bytes: number("maximumBytes")?,
            },
            Terms {
                price: number("price")?,
                maximum_path_bytes: number("maximumPathBytes")?,
                maximum_resource_bytes: number("maximumResourceBytes")?,
            },
        ))
    }

    // The effect an operation key was prepared with, or the reason there is none.
    fn prepared(&self, key: OperationKey) -> Result<Effect, Error> {
        match self.journal.get(key) {
            Some(Entry::Prepared { effect, .. }) => Ok(effect),
            _ => Err(Error::Denied),
        }
    }

    fn staged(&self) -> std::sync::MutexGuard<'_, BTreeMap<OperationKey, Completion>> {
        self.staged.lock().unwrap_or_else(|e| e.into_inner())
    }
}

// The token bytes of a participant token blob.
fn token(blob: &Blob) -> Result<[u8; 32], Error> {
    blob.bytes().try_into().map_err(|_| Error::Invalid)
}

impl services::handler::Provider for PageProvider {
    // Validate and describe one request: only the publisher's own provided offering is served, the request must fit it, and the page root must be usable.
    fn assess(&self, peer: Peer, offering: &Offering, input: &Value) -> Result<Assessment, Error> {
        tracing::trace!("Assessing a Page get request");
        if peer.local != self.identity
            || peer.remote != peer.key.requester
            || offering.direction != Direction::Provided
        {
            return Err(Error::Denied);
        }
        let terms = Terms::parse(offering).map_err(services::Error::from)?;
        self.request_for(&terms, input)?;
        // An offering whose root is missing, not HTML or too large is unusable and is not served.
        self.pages
            .check_root(&terms)
            .map_err(|_| Error::Unavailable)?;
        let budget = self.bounded(Budget {
            depth: usize::MAX,
            nodes: usize::MAX,
            bytes: usize::MAX,
        });
        let input_bytes = u64::try_from(input.byte_len(budget)?).map_err(|_| Error::Limit)?;
        Ok(Assessment {
            metadata: input.clone(),
            factors: Value::Object(vec![]),
            input_bytes,
            pin: None,
        })
    }

    // Validate the request against the accepted offering, pin one snapshot, journal the effect as prepared and return the staged result. A repeated call for the
    // same key returns the same staged result and never takes a second snapshot.
    fn execute(&self, invocation: &Invocation, input: &Value) -> Result<Completion, Error> {
        tracing::trace!("Staging an accepted Page get");
        let key = invocation.key;
        if let Some(prior) = self.staged().get(&key) {
            return Ok(prior.clone());
        }
        let terms = Terms::parse(&invocation.offering).map_err(services::Error::from)?;
        let request = self.request_for(&terms, input)?;
        let output = self.output_for(&terms, &request)?;
        let effect = Self::effect_for(&request, &terms);
        let mut token_bytes = [0u8; 32];
        self.entropy
            .fill(&mut token_bytes)
            .map_err(|_| Error::Unavailable)?;
        self.journal
            .put(
                key,
                Entry::Prepared {
                    effect: effect.clone(),
                    token: token_bytes,
                },
            )
            .map_err(services::Error::from)?;
        let completion = self.completion_of(&output, &effect)?;
        self.staged().insert(key, completion.clone());
        Ok(completion)
    }

    // The staged completion: the pinned snapshot while the process lives, otherwise rebuilt from the journaled request and terms (a resource that changed in the
    // meantime then fails the coordinator's comparison instead of being silently substituted).
    fn completion(&self, key: OperationKey) -> Result<Completion, Error> {
        tracing::trace!("Recovering a staged Page completion");
        if let Some(prior) = self.staged().get(&key) {
            return Ok(prior.clone());
        }
        let effect = self.prepared(key).map_err(|_| Error::Unknown)?;
        let (request, terms) = Self::decode_effect(&effect)?;
        let output = self.output_for(&terms, &request)?;
        let completion = self.completion_of(&output, &effect)?;
        self.staged().insert(key, completion.clone());
        Ok(completion)
    }

    // The effect is already durably prepared; confirm it is exactly the staged one and return its token.
    fn prepare(&self, key: OperationKey, effect: &Blob) -> Result<Blob, Error> {
        tracing::trace!("Preparing the staged Page effect");
        match self.journal.get(key) {
            Some(Entry::Prepared {
                effect: prepared,
                token: bytes,
            }) => {
                if prepared.encode() != effect.bytes() {
                    return Err(Error::Conflict);
                }
                Blob::new(bytes.to_vec(), 32)
            }
            _ => Err(Error::Denied),
        }
    }

    // A result must equal the original staged work exactly.
    fn validate_local_result(
        &self,
        invocation: &Invocation,
        _input: &Value,
        result: &Completion,
    ) -> Result<bool, Error> {
        tracing::trace!("Validating a Page result against the original staged work");
        let original = self.completion(invocation.key)?;
        if original.output != result.output
            || original.result != result.result
            || original.price != result.price
            || original.effect != result.effect
        {
            return Err(Error::Conflict);
        }
        Ok(true)
    }

    // Commit as prepared: the journal records the outcome so a lost reply is answered from it, and the pinned snapshot is released.
    fn commit(&self, key: OperationKey, token_blob: &Blob) -> Result<(), Error> {
        tracing::trace!("Committing a Page get");
        match self.journal.get(key) {
            None => Err(Error::Denied),
            Some(Entry::Aborted) => Err(Error::Conflict),
            Some(Entry::Committed(_)) => Ok(()),
            Some(Entry::Prepared {
                token: expected, ..
            }) if expected != token(token_blob)? => Err(Error::Denied),
            Some(Entry::Prepared { .. }) => {
                self.journal
                    .put(key, Entry::Committed(Outcome::Query))
                    .map_err(services::Error::from)?;
                self.staged().remove(&key);
                Ok(())
            }
        }
    }

    // Release a prepared effect only on an explicit abort; aborting an absent or already aborted operation is a no-op.
    fn abort(&self, key: OperationKey, token_blob: Option<&Blob>) -> Result<(), Error> {
        tracing::trace!("Aborting a Page get");
        match self.journal.get(key) {
            None | Some(Entry::Aborted) => Ok(()),
            Some(Entry::Committed(_)) => Err(Error::Conflict),
            Some(Entry::Prepared {
                token: expected, ..
            }) => {
                if token_blob
                    .map(token)
                    .transpose()?
                    .is_some_and(|given| given != expected)
                {
                    return Err(Error::Denied);
                }
                self.journal
                    .put(key, Entry::Aborted)
                    .map_err(services::Error::from)?;
                self.staged().remove(&key);
                Ok(())
            }
        }
    }

    // The durable state of an operation.
    fn status(&self, key: OperationKey) -> Result<Status, Error> {
        tracing::trace!("Reading the Page operation status");
        Ok(match self.journal.get(key) {
            None => Status::Absent,
            Some(Entry::Prepared { token, .. }) => Status::Prepared(Blob::new(token.to_vec(), 32)?),
            Some(Entry::Committed(_)) => Status::Committed,
            Some(Entry::Aborted) => Status::Aborted,
        })
    }

    // The Page check is local to the visitor: there is no remote evidence to serve.
    fn evidence(&self, _invocation: &CheckInvocation, _input: &Value) -> Result<Value, Error> {
        tracing::trace!("Refusing remote evidence for the local Page check");
        Err(Error::Unsupported)
    }
}
