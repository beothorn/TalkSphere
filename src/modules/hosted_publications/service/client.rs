//! The hosted publication requester contract and economic rules, installed for both the requester and the provider role.
//!
//! Services validates quotes, inputs, results and paid checks through one installed requester per contract. This client holds the pinned
//! definition, derives the same body-free projections the provider uses, recalculates every price from the offering and the requester's
//! own knowledge, and judges results and byte checks. What a requester independently knows about a placement (its content size, learned
//! when it stored it) comes through the [`Placements`] port, so a provider can never choose the size it is paid for on a renewal. A get is
//! priced from the exact size the provider states, which the result must then confirm byte for byte, and which can never exceed the
//! `maximumBytes` the requester chose; the publisher's signature is verified before a result is accepted.
//! Dependencies: terms and pricing, the codec, the check verdicts and the Services contracts.
use super::project::{self, number};
use crate::modules::{
    board::{
        definition::Definition,
        offering::{Direction, Offering},
    },
    hosted_publications::{
        check::{self, Expectation, Verdict},
        codec::{self, Request, Response},
        ids::PlacementId,
        standard::Catalogue,
        terms::Terms,
        verify::{Bounds, verify},
    },
    identity::api::IdentityId,
    services::{
        Error,
        checks::{Invocation as CheckInvocation, Mode, Rules, Timing},
        contract::OperationKey,
        handler::{Blob, Check, Invocation, Peer, Requester},
        pricing::Price,
        registry::Standards,
        replay::{Id as ReplayId, Purpose},
        validation::{self, Semantics},
        value::{Budget, Value},
    },
};
use std::sync::Arc;

/// What the requester independently knows about placements, never taken from a provider's reply.
pub trait Placements: Send + Sync {
    /// The content size of `placement` at `provider` as this requester knows it (from its own store).
    /// # Errors
    /// `Denied` when the requester knows no such placement; other errors for unavailable local state.
    fn content_size(
        &self,
        key: OperationKey,
        provider: IdentityId,
        placement: PlacementId,
    ) -> Result<u64, Error>;
}

/// The requester implementation; construction registers nothing and publishes nothing.
pub struct Client {
    definition: Definition,
    placements: Arc<dyn Placements>,
    values: Budget,
}

impl Client {
    /// Build the client for a definition the catalogue recognizes as the pinned hosted publication contract.
    /// # Errors
    /// `Invalid` for unusable value bounds, `Unsupported` or `Invalid` when the catalogue does not recognize the definition.
    pub fn new(
        definition: Definition,
        catalogue: &Catalogue,
        placements: Arc<dyn Placements>,
        values: Budget,
    ) -> Result<Self, Error> {
        tracing::trace!("Constructing the hosted publication requester client");
        if values.depth == 0 || values.depth > 31 || values.nodes == 0 || values.bytes == 0 {
            return Err(Error::Invalid);
        }
        catalogue.validate(&definition)?;
        Ok(Self {
            definition,
            placements,
            values,
        })
    }

    // Both budgets apply; neither can enlarge the other.
    fn budget(&self, caller: Budget) -> Budget {
        Budget {
            depth: caller.depth.min(self.values.depth),
            nodes: caller.nodes.min(self.values.nodes),
            bytes: caller.bytes.min(self.values.bytes),
        }
    }

    // The exact terms of a provided offering; the issuer is the publisher by the offering rules.
    fn parsed(&self, offering: &Offering) -> Result<Terms, Error> {
        Terms::parse(offering, offering.issuer).map_err(project::map)
    }

    // Recalculate a quote from the projection, the provider factor and the requester's own knowledge.
    fn price(
        &self,
        peer: Peer,
        offering: &Offering,
        metadata: &Value,
        factors: &Value,
    ) -> Result<Price, Error> {
        tracing::trace!("Pricing a hosted publication quote");
        let (_, provider) = roles(peer)?;
        let terms = self.parsed(offering)?;
        metadata.bounded(self.values)?;
        factors.bounded(self.values)?;
        let empty = matches!(factors, Value::Object(fields) if fields.is_empty());
        // The single factor a provider states, which must be the only member.
        let one_factor = |name: &str| -> Result<u64, Error> {
            if !matches!(factors, Value::Object(fields) if fields.len() == 1) {
                return Err(Error::Invalid);
            }
            number(factors, name)
        };
        match project::operation(metadata)? {
            "store" if empty => {
                let size = number(metadata, "contentSize")?;
                let seconds = number(metadata, "retentionSeconds")?;
                if size == 0
                    || size > terms.maximum_content_bytes
                    || seconds < terms.minimum_retention_seconds
                    || seconds > terms.maximum_retention_seconds
                {
                    return Err(Error::Invalid);
                }
                terms
                    .store_price(size, seconds)
                    .map(Price::Fixed)
                    .map_err(project::map)
            }
            "list" if empty => {
                let Request::List(request) = parse(metadata)? else {
                    return Err(Error::Invalid);
                };
                if !request.kind.is_listed() || request.limit > terms.maximum_list_results {
                    return Err(Error::Invalid);
                }
                Ok(Price::Fixed(terms.list_price))
            }
            "get" => {
                let Request::Get(request) = parse(metadata)? else {
                    return Err(Error::Invalid);
                };
                // The size the provider will return, never above what the requester allowed.
                let size = one_factor("contentSize")?;
                if size == 0 || size > request.maximum_bytes || size > terms.maximum_content_bytes {
                    return Err(Error::Invalid);
                }
                terms
                    .get_price(size)
                    .map(Price::Fixed)
                    .map_err(project::map)
            }
            "renew" => {
                let Request::Renew(request) = parse(metadata)? else {
                    return Err(Error::Invalid);
                };
                let claimed = one_factor("contentSize")?;
                if self
                    .placements
                    .content_size(peer.key, provider, request.placement_id)?
                    != claimed
                {
                    return Err(Error::Invalid);
                }
                terms
                    .renew_price(claimed, request.additional_seconds)
                    .map(Price::Fixed)
                    .map_err(project::map)
            }
            "remove" => {
                let Request::Remove(request) = parse(metadata)? else {
                    return Err(Error::Invalid);
                };
                if one_factor("releaseCredits")? < request.minimum_release_credits {
                    return Err(Error::Invalid);
                }
                Ok(Price::Fixed(terms.remove_price))
            }
            _ => Err(Error::Invalid),
        }
    }
}

// Derive the application requester and provider from the authenticated association, independently of stream direction.
fn roles(peer: Peer) -> Result<(IdentityId, IdentityId), Error> {
    tracing::trace!("Deriving hosted publication roles from the authenticated association");
    let requester = peer.key.requester;
    if peer.key.stream == 0 {
        return Err(Error::Denied);
    }
    let provider = if peer.local == requester {
        peer.remote
    } else if peer.remote == requester {
        peer.local
    } else {
        return Err(Error::Denied);
    };
    Ok((requester, provider))
}

// Decode a request value, mapping failures to the Services category.
fn parse(value: &Value) -> Result<Request, Error> {
    codec::parse_request(value).map_err(project::map)
}

impl Semantics for Client {
    // Apply the pinned operation rules to this installed contract's own descriptors only.
    fn validate(&self, descriptor: &serde_json::Value, value: &Value) -> Result<(), Error> {
        tracing::trace!("Applying hosted publication contract semantics");
        let definition = self.definition.document.value();
        value.bounded(self.values)?;
        if descriptor == &definition["input"] {
            parse(value).map(|_| ())
        } else if descriptor == &definition["output"] {
            codec::parse_response(value)
                .map(|_| ())
                .map_err(project::map)
        } else if descriptor == &definition["check"]["input"] {
            codec::parse_check_input(value)
                .map(|_| ())
                .map_err(project::map)
        } else if descriptor == &definition["check"]["output"] {
            check_output(value)
        } else {
            Err(Error::Unsupported)
        }
    }

    // No hosted publication operation interprets exceptional floating-point values.
    fn nonfinite(&self) -> bool {
        tracing::trace!("Declaring finite hosted publication value semantics");
        false
    }
}

// The requester-owned logical outcome of a check: a verdict and the challenged placement, with optional one-byte evidence.
fn check_output(value: &Value) -> Result<(), Error> {
    tracing::trace!("Validating a hosted publication check conclusion");
    let Value::Object(fields) = value else {
        return Err(Error::Invalid);
    };
    let get = |name: &str| fields.iter().find(|(n, _)| n == name).map(|(_, v)| v);
    let ok = matches!(get("outcome"), Some(Value::String(o)) if matches!(o.as_str(), "pass" | "fail" | "inconclusive"))
        && matches!(get("placementId"), Some(Value::Bytes(id)) if id.len() == 32)
        && matches!(get("offset"), Some(Value::Uint64(_)))
        && get("value").is_none_or(|v| matches!(v, Value::Bytes(b) if b.len() == 1));
    if ok { Ok(()) } else { Err(Error::Invalid) }
}

impl Requester for Client {
    // The exact definition installed with this code.
    fn definition(&self) -> &Definition {
        tracing::trace!("Borrowing the installed hosted publication definition");
        &self.definition
    }

    // Economic understanding is independent of any reservation or placement.
    fn terms(&self, offering: &Offering) -> Result<(), Error> {
        tracing::trace!("Validating hosted publication offering economics");
        self.parsed(offering).map(|_| ())
    }

    // Removals may be retransmitted over a new stream only by their own replay identifier; reads change nothing and have none.
    fn cached_retry(&self, input: &Value) -> Result<Option<ReplayId>, Error> {
        tracing::trace!("Identifying a hosted publication cached retry");
        Ok(match parse(input)? {
            Request::Remove(request) => Some(ReplayId {
                purpose: Purpose::Removal,
                placement: request.placement_id.0,
                bytes: request.removal_id.0,
            }),
            _ => None,
        })
    }

    // Prices need the authenticated provider and the requester's own placement knowledge.
    fn pricing_for(&self, _: &Offering, _: &Value, _: &Value) -> Result<Price, Error> {
        tracing::trace!("Refusing an unscoped hosted publication price");
        Err(Error::Unsupported)
    }

    // Local-provider and remote-provider quotes share one exact calculation.
    fn pricing_with_peer(
        &self,
        peer: Peer,
        offering: &Offering,
        metadata: &Value,
        factors: &Value,
    ) -> Result<Price, Error> {
        tracing::trace!("Calculating a scoped hosted publication price");
        self.price(peer, offering, metadata, factors)
    }

    // The body-free projection holds only independently known counts.
    fn assessment_metadata(&self, input: &Value, budget: Budget) -> Result<Value, Error> {
        tracing::trace!("Deriving hosted publication metadata without the content");
        project::metadata(input, self.budget(budget))
    }

    // Validate the current terms while projecting, retaining the authenticated association for the scoped price.
    fn assessment_metadata_with_peer(
        &self,
        peer: Peer,
        offering: &Offering,
        input: &Value,
        budget: Budget,
    ) -> Result<Value, Error> {
        tracing::trace!("Deriving hosted publication metadata under the original association");
        roles(peer)?;
        self.parsed(offering)?;
        project::metadata(input, self.budget(budget))
    }

    // No context-free factor validation can infer the provider.
    fn validate_assessment(&self, _: &Value, _: &Value, _: &Value, _: Budget) -> Result<(), Error> {
        tracing::trace!("Refusing unscoped hosted publication factor validation");
        Err(Error::Unsupported)
    }

    // The provider's metadata must be exactly the requester's own projection and its factors must price correctly.
    fn validate_assessment_with_peer(
        &self,
        peer: Peer,
        offering: &Offering,
        input: &Value,
        metadata: &Value,
        factors: &Value,
        budget: Budget,
    ) -> Result<(), Error> {
        tracing::trace!("Validating hosted publication quote expectations");
        let budget = self.budget(budget);
        metadata.bounded(budget)?;
        factors.bounded(budget)?;
        if self.assessment_metadata_with_peer(peer, offering, input, budget)? != *metadata {
            return Err(Error::Invalid);
        }
        self.price(peer, offering, metadata, factors).map(|_| ())
    }

    // The complete body must match the accepted projection; no placeholder body can pass.
    fn validate_input(&self, metadata: &Value, input: &Value, budget: Budget) -> Result<(), Error> {
        tracing::trace!(
            "Checking complete hosted publication input against the accepted projection"
        );
        let budget = self.budget(budget);
        validation::complete(
            input,
            &self.definition.document.value()["input"],
            budget,
            self,
        )?;
        metadata.bounded(budget)?;
        if project::metadata(input, budget)? != *metadata {
            return Err(Error::Invalid);
        }
        Ok(())
    }

    // A result must belong to the request that was made. For a get the publisher's signature is verified before anything is accepted and
    // the returned size must be exactly the size quoted; the receiving client interprets restricted content itself.
    fn validate_result(
        &self,
        invocation: &Invocation,
        input: &Value,
        output: &Value,
    ) -> Result<(), Error> {
        tracing::trace!("Checking a hosted publication result against its request");
        let request = parse(input)?;
        let response = codec::parse_response(output).map_err(project::map)?;
        let consistent = match (&request, &response) {
            (Request::Store(r), Response::Store(o, _)) => {
                o.publication_id == r.publication.metadata.publication_id
                    && o.accepted_at.checked_add(r.retention_seconds) == Some(o.expires_at)
            }
            (Request::List(r), Response::List(o, _)) => {
                o.publisher_id == r.publisher_id
                    && o.entries.len() <= r.limit as usize
                    && o.entries.iter().all(|e| e.kind == r.kind)
                    && ordered_after(&o.entries, r.before)
            }
            (Request::Get(r), Response::Get(o, _)) => {
                let m = &o.publication.metadata;
                let terms = self.parsed(&invocation.offering)?;
                let bounds = Bounds {
                    maximum_audience: usize::from(terms.maximum_audience),
                    maximum_content_bytes: terms.maximum_content_bytes,
                };
                m.publication_id == r.publication_id
                    && m.publisher_id == r.publisher_id
                    && m.content_size <= r.maximum_bytes
                    && number(&invocation.factors, "contentSize") == Ok(m.content_size)
                    && verify(&o.publication, r.publisher_id, bounds).is_ok()
            }
            (Request::Renew(r), Response::Renew(o, _)) => {
                o.placement_id == r.placement_id
                    && o.previous_expires_at == r.expected_expires_at
                    && r.expected_expires_at.checked_add(r.additional_seconds) == Some(o.expires_at)
            }
            (Request::Remove(r), Response::Remove(o)) => {
                o.placement_id == r.placement_id
                    && o.removal_id == r.removal_id
                    && o.removed
                    && o.release_credits >= r.minimum_release_credits
            }
            _ => false,
        };
        if consistent {
            Ok(())
        } else {
            Err(Error::Invalid)
        }
    }

    // Hosted publications have separately paid remote byte checks.
    fn checks(&self) -> Option<&dyn Rules> {
        tracing::trace!("Returning the hosted publication paid check rules");
        Some(self)
    }

    // Publication-level verification (signatures, decryption) is done by the receiving client, so no extra conclusion exists here.
    fn verify(
        &self,
        _input: &Value,
        _terms: &Blob,
        _result: &Value,
        _expectations: Option<&Value>,
    ) -> Result<Check, Error> {
        tracing::trace!("Declining a context-free hosted publication verification");
        Ok(Check::Inconclusive)
    }
}

// Whether listed entries are in strictly descending `(sequence, publicationId)` order and strictly after the request cursor.
fn ordered_after(
    entries: &[crate::modules::hosted_publications::operation::ListEntry],
    before: Option<(u64, crate::modules::hosted_publications::ids::PublicationId)>,
) -> bool {
    let keys: Vec<_> = entries
        .iter()
        .map(|e| (e.sequence, e.publication_id))
        .collect();
    keys.windows(2).all(|pair| pair[0] > pair[1])
        && keys
            .first()
            .is_none_or(|first| before.is_none_or(|cursor| *first < cursor))
}

impl Rules for Client {
    // Every challenge is separately paid at the current provided terms.
    fn mode(&self, offering: &Offering, challenge: &Value) -> Result<Mode, Error> {
        tracing::trace!("Selecting the paid hosted publication check mode");
        self.shape(challenge)?;
        if offering.direction != Direction::Provided {
            return Err(Error::Denied);
        }
        self.parsed(offering)?;
        Ok(Mode::Paid)
    }

    // Convert the offered seconds to the bounded local millisecond contract.
    fn response_milliseconds(&self, offering: &Offering) -> Result<u64, Error> {
        tracing::trace!("Reading the hosted publication check response interval");
        self.parsed(offering)?
            .check_response_seconds
            .checked_mul(1000)
            .ok_or(Error::Limit)
    }

    // Validate only the public challenge shape.
    fn shape(&self, challenge: &Value) -> Result<(), Error> {
        tracing::trace!("Validating a hosted publication challenge shape");
        validation::validate(
            challenge,
            &self.definition.document.value()["check"]["input"],
            self.values,
            false,
        )?;
        codec::parse_check_input(challenge)
            .map(|_| ())
            .map_err(project::map)
    }

    // The challenged offset must lie inside the content the requester knows it stored.
    fn challenge(&self, invocation: &CheckInvocation, challenge: &Value) -> Result<(), Error> {
        tracing::trace!("Authorizing a hosted publication challenge");
        self.shape(challenge)?;
        if !self.definition.same_contract(&invocation.definition)
            || invocation.key.requester != invocation.requester
        {
            return Err(Error::Invalid);
        }
        let terms = self.parsed(&invocation.offering)?;
        if invocation.price != terms.check_price
            || invocation.response_milliseconds
                != terms
                    .check_response_seconds
                    .checked_mul(1000)
                    .ok_or(Error::Limit)?
        {
            return Err(Error::Invalid);
        }
        let challenge = codec::parse_check_input(challenge).map_err(project::map)?;
        let size = self.placements.content_size(
            invocation.key,
            invocation.provider,
            challenge.placement_id,
        )?;
        if challenge.offset >= size {
            return Err(Error::Invalid);
        }
        Ok(())
    }

    // Provider evidence carries one byte and never the requester-owned conclusion.
    fn evidence_schema(&self) -> Result<serde_json::Value, Error> {
        tracing::trace!("Describing hosted publication provider evidence");
        Ok(serde_json::json!({"type":"object","fields":{
            "placementId":{"type":"bytes","description":"Placement that was checked."},
            "offset":{"type":"uint64","description":"Challenged content offset."},
            "value":{"type":"bytes","description":"Exactly one provider evidence byte."}
        }}))
    }

    // Evidence must answer exactly this challenge; its correctness is judged separately.
    fn evidence(&self, challenge: &Value, evidence: &Value) -> Result<(), Error> {
        tracing::trace!("Checking hosted publication evidence against its challenge");
        self.shape(challenge)?;
        let challenge = codec::parse_check_input(challenge).map_err(project::map)?;
        let evidence = codec::parse_check_output(evidence).map_err(project::map)?;
        if check::is_valid_evidence(&challenge, &evidence) {
            Ok(())
        } else {
            Err(Error::Invalid)
        }
    }

    // Pass or fail only from complete, timely, matching evidence and a private expectation for this challenge.
    fn outcome(
        &self,
        challenge: &Value,
        evidence: Option<&Value>,
        expectations: Option<&Value>,
        timing: Timing,
    ) -> Check {
        tracing::trace!("Judging a hosted publication byte check");
        let (Ok(challenge), Some(evidence), Some(expectations)) = (
            codec::parse_check_input(challenge),
            evidence.and_then(|e| codec::parse_check_output(e).ok()),
            expectations.and_then(expectation),
        ) else {
            return Check::Inconclusive;
        };
        let timing = match timing {
            Timing::Within(_) => check::Timing::Within,
            Timing::Ambiguous => check::Timing::Ambiguous,
            Timing::Missing => check::Timing::Missing,
        };
        match check::evaluate(&challenge, Some(&evidence), Some(&expectations), timing) {
            Verdict::Pass => Check::Pass,
            Verdict::Fail => Check::Fail,
            Verdict::Inconclusive => Check::Inconclusive,
        }
    }
}

// The requester-private expectation value: `{placementId, offset, expectedByte}`.
fn expectation(value: &Value) -> Option<Expectation> {
    let Value::Object(fields) = value else {
        return None;
    };
    let get = |name: &str| fields.iter().find(|(n, _)| n == name).map(|(_, v)| v);
    let (Some(Value::Bytes(id)), Some(Value::Uint64(offset)), Some(Value::Bytes(byte))) =
        (get("placementId"), get("offset"), get("expectedByte"))
    else {
        return None;
    };
    let ([byte], Ok(id)) = (byte.as_slice(), <[u8; 32]>::try_from(id.as_slice())) else {
        return None;
    };
    Some(Expectation::new(PlacementId(id), *offset, *byte))
}
