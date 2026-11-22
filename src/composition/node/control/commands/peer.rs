//! Paid Storage requests to a peer: fetch offerings, store, retrieve, renew, check and remove.
//!
//! Each request resolves an authenticated session through the node loop, spends only under the
//! caller's explicit bound, waits for the normally ended exchange, reads the committed result
//! through Services and validates it with the Storage requester contracts before the catalog
//! changes. A lost reply is never retried: it is reported as an unknown outcome.
use super::{Handler, budget, now, service_error, unavailable};
use crate::{
    composition::node::{
        catalog::Entry,
        control::protocol::{Code, ControlError, Request, hex},
        offering::{OFFERING_ID, WANTED_ID},
    },
    composition::policy::Event,
    modules::{
        identity::{
            api::{Contact, IdentityId},
            contact, text,
        },
        peer_protocol::{
            self as peer,
            engine::endpoint::{
                cache::PeerCache,
                requests::{Ceiling, Challenge, Invocation, Plan, Reply},
                wanted::Invitation,
            },
        },
        services::{
            self,
            contract::OperationKey,
            handler::{Check, Completion},
            value::Value as Typed,
        },
        storage::{
            pricing,
            requester::{self, catalog::remember},
            terms::Terms,
        },
    },
};
use serde_json::{Value, json};
use std::{sync::Arc, time::Instant};

/// Authority lifetime budget for local Services observation grants.
const OBSERVE_BYTES: usize = 16_777_216;
/// Computes an exact fixed price from the selected terms.
type Fixed = dyn Fn(&Terms) -> Result<u64, ControlError>;
/// Longest accepted contact text.
const CONTACT_BYTES: usize = 1024;

impl Handler {
    // Run one peer operation and journal how it ended; the journal never holds payload or peer text.
    pub(super) fn peer_operation(&self, request: &Request) -> Result<Value, ControlError> {
        tracing::trace!("Running a peer Storage operation");
        let start = Instant::now();
        let mut lease = None;
        let result = match request.op.as_str() {
            "fetch" => self.fetch(request, start),
            "fulfil" => self.fulfil(request, start),
            "store" => self.store(request, start, &mut lease),
            "retrieve" => self.retrieve(request, start, &mut lease),
            "renew" => self.renew(request, start, &mut lease),
            "check" => self.check(request, start, &mut lease),
            _ => self.remove(request, start, &mut lease),
        };
        self.note(request, lease.as_ref(), &result);
        result
    }

    // Authenticate and fetch the peer's Board, returning its public provided offerings. The Storage
    // definition is fetched too, and the session's cache retained, only when the peer offers Storage.
    fn fetch(&self, request: &Request, start: Instant) -> Result<Value, ControlError> {
        tracing::trace!("Fetching a peer Board");
        let contact = parse_contact(request.text("contact")?)?;
        let peer_id = contact.expected_id;
        let address = contact.endpoint;
        self.link
            .connect(peer_id, address, self.remaining(start)?)
            .map_err(|m| ControlError::new(Code::Unavailable, m))?;
        let cache = self.fresh_cache()?;
        let reply = self.exchange(&contact, peer_id, Plan::Board(cache.clone()), start, false)?;
        let Reply::Metadata(remote) = reply else {
            return Err(ControlError::new(Code::Refused, "the peer sent no Board"));
        };
        let board: Value = serde_json::from_slice(remote.bytes())
            .map_err(|_| ControlError::new(Code::Refused, "the peer Board is not valid JSON"))?;
        let provided = board["providedServices"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if provided.iter().any(|o| o["id"] == OFFERING_ID) {
            let plan = Plan::Definition {
                cache: cache.clone(),
                offering: OFFERING_ID.into(),
            };
            self.exchange(&contact, peer_id, plan, start, false)?;
            self.link.retain_cache(peer_id, cache);
        }
        Ok(json!({"publisher": text::format(peer_id), "provided": provided}))
    }

    // Offer to fulfil the peer's wanted Storage terms over a fresh authenticated session: the peer, as
    // requester, chooses the object and pays in its own credits, which this node records only after the
    // provider-side commit. Nothing is spent here; the reply shows the resulting balances.
    fn fulfil(&self, request: &Request, start: Instant) -> Result<Value, ControlError> {
        tracing::trace!("Offering to fulfil wanted Storage terms");
        let contact = parse_contact(request.text("contact")?)?;
        let peer_id = contact.expected_id;
        self.link
            .connect(peer_id, contact.endpoint, self.remaining(start)?)
            .map_err(|m| ControlError::new(Code::Unavailable, m))?;
        let cache = self.fresh_cache()?;
        let reply = self.exchange(&contact, peer_id, Plan::Board(cache.clone()), start, false)?;
        let Reply::Metadata(remote) = reply else {
            return Err(ControlError::new(Code::Refused, "the peer sent no Board"));
        };
        let board: Value = serde_json::from_slice(remote.bytes())
            .map_err(|_| ControlError::new(Code::Refused, "the peer Board is not valid JSON"))?;
        let wanted = board["wantedServices"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if !wanted.iter().any(|o| o["id"] == WANTED_ID) {
            return Err(ControlError::new(
                Code::Refused,
                "the peer publishes no wanted Storage offering",
            ));
        }
        let plan = Plan::Definition {
            cache: cache.clone(),
            offering: WANTED_ID.into(),
        };
        self.exchange(&contact, peer_id, plan, start, false)?;
        let selection = cache
            .selection(peer_id, WANTED_ID, "cr2se.storage", 1)
            .map_err(|_| ControlError::new(Code::Refused, "the wanted offering is not current"))?;
        let plan = Plan::Offer(Box::new(Invitation { selection, cache }));
        let Reply::Work(_) = self.exchange(&contact, peer_id, plan, start, true)? else {
            return Err(ControlError::new(
                Code::Refused,
                "the peer returned no work result",
            ));
        };
        Ok(json!({
            "publisher": text::format(peer_id), "fulfilled": true,
            "balance": self.balance_of(peer_id)?,
        }))
    }

    // Store a local object within an explicit spending bound and catalog the validated lease.
    fn store(
        &self,
        request: &Request,
        start: Instant,
        lease: &mut Option<[u8; 32]>,
    ) -> Result<Value, ControlError> {
        tracing::trace!("Storing an object at a peer");
        let contact = parse_contact(request.text("contact")?)?;
        let data = request.bytes(
            "data",
            crate::composition::node::config::MAXIMUM_OBJECT_BYTES as usize,
        )?;
        let retention = request.u64("retentionSeconds")?;
        let maximum = request.u64("maximum")?;
        if data.is_empty() {
            return Err(ControlError::new(Code::Invalid, "data must not be empty"));
        }
        let hash = crate::modules::cryptography::primitives::hash::sha256(&data, data.len())
            .map_err(|_| ControlError::new(Code::Limit, "the object could not be hashed"))?;
        let input = fields([
            ("operation", Typed::String("store".into())),
            ("sizeBytes", Typed::Uint64(data.len() as u64)),
            ("retentionSeconds", Typed::Uint64(retention)),
            ("contentHash", Typed::Bytes(hash.to_vec())),
            ("data", Typed::Bytes(data)),
        ]);
        let key = self.invoke(&contact, input.clone(), maximum, None, start)?;
        let observe = self.observe_grant()?;
        let context = self.context(&observe, start)?;
        let validated = remember(
            &requester::catalog::Services {
                coordinator: &self.owners.runtime,
                context: &context,
            },
            self.owners.catalog.as_ref(),
            key,
            &input,
            budget(),
            requester::store::Policy {
                observed_unix_seconds: now(),
                maximum_clock_difference_seconds: 60,
            },
        )
        .map_err(service_error)?;
        *lease = Some(validated.id);
        self.owners
            .catalog
            .put(&validated, &contact::format(contact).unwrap_or_default())
            .map_err(|_| unavailable("the lease catalog"))?;
        Ok(json!({
            "leaseId": hex(&validated.id), "provider": text::format(validated.provider),
            "sizeBytes": validated.size_bytes, "contentHash": hex(&validated.content_hash),
            "acceptedAt": validated.accepted_at, "expiresAt": validated.expires_at,
            "price": validated.store_price,
        }))
    }

    // Retrieve a range; data is returned only after the Storage requester contract validated it.
    fn retrieve(
        &self,
        request: &Request,
        start: Instant,
        lease: &mut Option<[u8; 32]>,
    ) -> Result<Value, ControlError> {
        tracing::trace!("Retrieving a range from a peer");
        let entry = self.entry(request)?;
        *lease = Some(entry.lease.id);
        let offset = request.optional_u64("offset")?.unwrap_or(0);
        let length = request
            .optional_u64("length")?
            .unwrap_or(entry.lease.size_bytes.saturating_sub(offset));
        let maximum = request.u64("maximum")?;
        let input = fields([
            ("operation", Typed::String("retrieve".into())),
            ("leaseId", Typed::Bytes(entry.lease.id.to_vec())),
            ("offset", Typed::Uint64(offset)),
            ("length", Typed::Uint64(length)),
        ]);
        let contact = entry_contact(&entry)?;
        let price = move |terms: &Terms| {
            pricing::usage(length, 1, terms.retrieval, terms.minimum_price).map_err(service_error)
        };
        let key = self.invoke(&contact, input.clone(), maximum, Some(&price), start)?;
        let (invocation, completion) = self.completed(key, start)?;
        let retrieved = requester::retrieve::validate(
            &invocation,
            &input,
            &completion.output,
            &entry.lease,
            None,
            budget(),
        )
        .map_err(service_error)?;
        Ok(json!({
            "leaseId": hex(&entry.lease.id), "offset": offset, "length": length,
            "data": hex(retrieved.data), "wholeObjectHashChecked": retrieved.whole_object_hash_checked,
            "price": completion.price,
        }))
    }

    // Renew with the cataloged expected expiry and update the catalog from the validated result.
    fn renew(
        &self,
        request: &Request,
        start: Instant,
        lease: &mut Option<[u8; 32]>,
    ) -> Result<Value, ControlError> {
        tracing::trace!("Renewing a lease at a peer");
        let entry = self.entry(request)?;
        *lease = Some(entry.lease.id);
        let seconds = request.u64("seconds")?;
        let maximum = request.u64("maximum")?;
        let expected = request
            .optional_u64("expectedExpiresAt")?
            .unwrap_or(entry.lease.expires_at);
        let input = fields([
            ("operation", Typed::String("renew".into())),
            ("leaseId", Typed::Bytes(entry.lease.id.to_vec())),
            ("expectedExpiresAt", Typed::Uint64(expected)),
            ("additionalSeconds", Typed::Uint64(seconds)),
        ]);
        let contact = entry_contact(&entry)?;
        let size = entry.lease.size_bytes;
        let price = move |terms: &Terms| {
            pricing::usage(size, seconds, terms.renewal, terms.minimum_price).map_err(service_error)
        };
        let key = self.invoke(&contact, input.clone(), maximum, Some(&price), start)?;
        let (invocation, completion) = self.completed(key, start)?;
        let updated = requester::renew::validate(
            &invocation,
            &input,
            &completion.output,
            &entry.lease,
            budget(),
        )
        .map_err(service_error)?;
        self.owners
            .catalog
            .put(&updated, &entry.contact)
            .map_err(|_| unavailable("the lease catalog"))?;
        Ok(json!({
            "leaseId": hex(&updated.id), "previousExpiresAt": entry.lease.expires_at,
            "expiresAt": updated.expires_at, "price": completion.price,
        }))
    }

    // Remove a lease; the catalog records it removed only after the result validates.
    fn remove(
        &self,
        request: &Request,
        start: Instant,
        lease: &mut Option<[u8; 32]>,
    ) -> Result<Value, ControlError> {
        tracing::trace!("Removing a lease at a peer");
        let entry = self.entry(request)?;
        *lease = Some(entry.lease.id);
        let maximum = request.u64("maximum")?;
        let input = fields([
            ("operation", Typed::String("remove".into())),
            ("leaseId", Typed::Bytes(entry.lease.id.to_vec())),
        ]);
        let contact = entry_contact(&entry)?;
        let price = |terms: &Terms| Ok(terms.remove_price);
        let key = self.invoke(&contact, input.clone(), maximum, Some(&price), start)?;
        let (invocation, completion) = self.completed(key, start)?;
        let removed = requester::remove::validate(
            &invocation,
            &input,
            &completion.output,
            &entry.lease,
            budget(),
        )
        .map_err(service_error)?;
        self.owners
            .catalog
            .put(&removed, &entry.contact)
            .map_err(|_| unavailable("the lease catalog"))?;
        Ok(json!({"leaseId": hex(&removed.id), "removed": true, "price": completion.price}))
    }

    // Paid byte check with a private expected value that never leaves this node.
    fn check(
        &self,
        request: &Request,
        start: Instant,
        lease: &mut Option<[u8; 32]>,
    ) -> Result<Value, ControlError> {
        tracing::trace!("Checking a stored byte at a peer");
        let entry = self.entry(request)?;
        *lease = Some(entry.lease.id);
        let offset = request.u64("offset")?;
        let expected = request.bytes("expectedByte", 1)?;
        let maximum = request.u64("maximum")?;
        if expected.len() != 1 {
            return Err(ControlError::new(
                Code::Invalid,
                "expectedByte must be exactly one byte",
            ));
        }
        let contact = entry_contact(&entry)?;
        let (provider, cache) = self.cache(&contact, start, false)?;
        let selection = cache
            .selection(provider, OFFERING_ID, "cr2se.storage", 1)
            .map_err(|_| ControlError::new(Code::Refused, "the peer offering is not current"))?;
        let plan = Plan::Check(Box::new(Challenge {
            selection,
            cache: cache.clone(),
            challenge: fields([
                ("leaseId", Typed::Bytes(entry.lease.id.to_vec())),
                ("offset", Typed::Uint64(offset)),
            ]),
            spending: peer::requester::Spending {
                maximum,
                payment: Default::default(),
            },
            consent: Arc::new(Ceiling(maximum)),
        }));
        let Reply::Check(key) = self.exchange(&contact, provider, plan, start, true)? else {
            return Err(ControlError::new(
                Code::Refused,
                "the peer returned no check result",
            ));
        };
        let observe = self.observe_grant()?;
        let context = self.context(&observe, start)?;
        let expectations = fields([
            ("leaseId", Typed::Bytes(entry.lease.id.to_vec())),
            ("offset", Typed::Uint64(offset)),
            ("expectedByte", Typed::Bytes(expected)),
            ("active", Typed::Bool(true)),
        ]);
        let outcome = self
            .owners
            .runtime
            .check_outcome(&context, key, Some(&expectations))
            .map_err(service_error)?;
        // Only a conclusive result is evidence about the provider; an inconclusive check proves nothing.
        match outcome {
            Check::Pass => self.owners.observe(provider, Event::CheckPassed),
            Check::Fail => self.owners.observe(provider, Event::CheckFailed),
            Check::Inconclusive => {}
        }
        Ok(json!({
            "leaseId": hex(&entry.lease.id), "offset": offset,
            "outcome": match outcome { Check::Pass => "pass", Check::Fail => "fail", Check::Inconclusive => "inconclusive" },
        }))
    }

    // Look up the cataloged lease named by the request, requiring a provider when the ID is ambiguous.
    fn entry(&self, request: &Request) -> Result<Entry, ControlError> {
        tracing::trace!("Resolving a cataloged lease");
        let id: [u8; 32] = request
            .bytes("lease", 32)?
            .try_into()
            .map_err(|_| ControlError::new(Code::Invalid, "lease must be 32 bytes of hex"))?;
        let provider = match request.optional_text("provider")? {
            Some(text) => Some(Self::identity_text(text)?),
            None => None,
        };
        let mut found: Vec<Entry> = self
            .owners
            .catalog
            .find(id)
            .into_iter()
            .filter(|e| provider.is_none_or(|p| e.lease.provider == p))
            .collect();
        match found.len() {
            0 => Err(ControlError::new(Code::Denied, "no such lease")),
            1 => Ok(found.remove(0)),
            _ => Err(ControlError::new(
                Code::Invalid,
                "the lease id exists at several providers; give provider",
            )),
        }
    }

    // Parse an identity text.
    fn identity_text(text_value: &str) -> Result<IdentityId, ControlError> {
        tracing::trace!("Parsing a provider identity");
        text::parse(text_value)
            .map_err(|_| ControlError::new(Code::Invalid, "provider must be a canonical CR2SE ID"))
    }

    // Ensure an authenticated session and a fresh metadata cache for the contact.
    fn cache(
        &self,
        contact: &Contact,
        start: Instant,
        refresh: bool,
    ) -> Result<(IdentityId, Arc<PeerCache>), ControlError> {
        tracing::trace!("Ensuring a peer session and metadata cache");
        let address = contact.endpoint;
        let peer = contact.expected_id;
        let remaining = self.remaining(start)?;
        let session = self
            .link
            .connect(peer, address, remaining)
            .map_err(|m| ControlError::new(Code::Unavailable, m))?;
        if let Some(cache) = session.cache.clone().filter(|_| !refresh) {
            return Ok((peer, cache));
        }
        let cache = self.fresh_cache()?;
        for plan in [
            Plan::Board(cache.clone()),
            Plan::Definition {
                cache: cache.clone(),
                offering: OFFERING_ID.into(),
            },
        ] {
            self.exchange(contact, peer, plan, start, false)?;
        }
        self.link.retain_cache(peer, cache.clone());
        Ok((peer, cache))
    }

    // A new empty direct-publisher cache over the installed requester contracts.
    fn fresh_cache(&self) -> Result<Arc<PeerCache>, ControlError> {
        tracing::trace!("Creating a metadata cache");
        PeerCache::new(self.owners.registry.clone(), Default::default())
            .map(Arc::new)
            .map_err(|_| unavailable("the metadata cache"))
    }

    // Open one prepared plan on the peer's session and wait for its normally ended outcome.
    fn exchange(
        &self,
        contact: &Contact,
        peer_id: IdentityId,
        plan: Plan,
        start: Instant,
        paid: bool,
    ) -> Result<Reply, ControlError> {
        tracing::trace!("Running one peer exchange");
        let address = contact.endpoint;
        let remaining = self.remaining(start)?;
        let session = self
            .link
            .connect(peer_id, address, remaining)
            .map_err(|m| ControlError::new(Code::Unavailable, m))?;
        let (call, completed) = self.owners.endpoint.request(plan);
        self.link
            .open(&session.connection, Arc::new(call), self.remaining(start)?)
            .map_err(|m| {
                self.link.forget(peer_id);
                ControlError::new(Code::Unavailable, m)
            })?;
        // An exchange that was opened may have started remote work, so silence is unknown, never failure.
        let completion = completed
            .recv_timeout(self.remaining(start)?)
            .map_err(|_| {
                if paid {
                    ControlError::new(
                        Code::UnknownOutcome,
                        "no completion was observed; the outcome is unknown",
                    )
                } else {
                    ControlError::new(Code::Unavailable, "the peer did not answer in time")
                }
            })?;
        if let Err(error) = &completion.result {
            tracing::debug!(?error, cause = ?completion.cause, remote = ?completion.failure.as_ref().map(|f| f.code), "A peer exchange did not complete successfully");
        }
        // A local spending refusal at the quote is raised before acceptance, so no remote work can
        // exist even though the exchange did not end normally; report it as the known refusal it is.
        if matches!(
            (&completion.result, &completion.cause),
            (Err(peer::Error::Unknown), Some(peer::Error::Insufficient))
        ) {
            return Err(peer_error(peer::Error::Insufficient, paid));
        }
        completion.result.map_err(|error| peer_error(error, paid))
    }

    // Invoke one paid operation. Store spends up to the caller's ceiling; every other operation has an
    // exact fixed price that the provider requires as the quote maximum, so the node computes that price
    // from the selected terms and refuses when it exceeds the caller's ceiling.
    fn invoke(
        &self,
        contact: &Contact,
        input: Typed,
        limit: u64,
        fixed: Option<&Fixed>,
        start: Instant,
    ) -> Result<OperationKey, ControlError> {
        tracing::trace!("Invoking a paid peer operation");
        let (peer_id, cache) = self.cache(contact, start, false)?;
        let (selection, cache) = match cache.selection(peer_id, OFFERING_ID, "cr2se.storage", 1) {
            Ok(selection) => (selection, cache),
            // The cached terms went stale; fetch fresh ones once and select again.
            Err(_) => {
                let (_, refreshed) = self.cache(contact, start, true)?;
                let selection = refreshed
                    .selection(peer_id, OFFERING_ID, "cr2se.storage", 1)
                    .map_err(|_| {
                        ControlError::new(Code::Refused, "the peer offering is not current")
                    })?;
                (selection, refreshed)
            }
        };
        let maximum = match fixed {
            None => limit,
            Some(price) => {
                let terms = Terms::parse(selection.offering()).map_err(service_error)?;
                let exact = price(&terms)?;
                if exact > limit {
                    return Err(ControlError::new(
                        Code::Insufficient,
                        "the quoted price exceeds the spending bound",
                    ));
                }
                exact
            }
        };
        let plan = Plan::Invoke(Box::new(Invocation {
            selection,
            cache,
            input,
            spending: peer::requester::Spending {
                maximum,
                payment: Default::default(),
            },
            consent: Arc::new(Ceiling(maximum)),
        }));
        match self.exchange(contact, peer_id, plan, start, true)? {
            Reply::Work(key) => Ok(key),
            _ => Err(ControlError::new(
                Code::Refused,
                "the peer returned no work result",
            )),
        }
    }

    // Read the committed original invocation and output through a fresh Observe grant.
    fn completed(
        &self,
        key: OperationKey,
        start: Instant,
    ) -> Result<(services::handler::Invocation, Completion), ControlError> {
        tracing::trace!("Reading a committed result");
        let observe = self.observe_grant()?;
        let context = self.context(&observe, start)?;
        self.owners
            .runtime
            .completed(&context, key)
            .map_err(service_error)
    }

    // A scoped Observe grant for reading retained results.
    fn observe_grant(&self) -> Result<services::authority::Grant, ControlError> {
        tracing::trace!("Issuing a Services observation grant");
        self.owners
            .services
            .grant(
                self.owners.local,
                services::authority::Action::Observe,
                OBSERVE_BYTES,
            )
            .map_err(service_error)
    }

    // Services context bounded by the request deadline.
    fn context<'a>(
        &self,
        grant: &'a services::authority::Grant,
        start: Instant,
    ) -> Result<services::contract::Context<'a>, ControlError> {
        tracing::trace!("Building a Services context");
        Ok(services::contract::Context {
            version: 1,
            grant,
            deadline: Instant::now() + self.remaining(start)?,
            cancelled: false,
        })
    }
}

// Parse a contact; the address is optional because a peer with a live connection needs none.
fn parse_contact(text_value: &str) -> Result<Contact, ControlError> {
    tracing::trace!("Parsing a peer contact");
    let contact = contact::parse(text_value, CONTACT_BYTES).map_err(|_| {
        ControlError::new(
            Code::Invalid,
            "contact must be a canonical ID, optionally with address and port",
        )
    })?;
    Ok(contact)
}

// Rebuild the stored contact of a cataloged lease.
fn entry_contact(entry: &Entry) -> Result<Contact, ControlError> {
    tracing::trace!("Reading a cataloged contact");
    parse_contact(&entry.contact)
}

// Map a Peer Protocol failure; an unknown or interrupted paid exchange is never reported as refused.
fn peer_error(error: peer::Error, paid: bool) -> ControlError {
    tracing::trace!("Mapping a peer exchange failure");
    use peer::Error as E;
    match error {
        E::Unknown if paid => ControlError::new(
            Code::UnknownOutcome,
            "the remote effect may have committed; the outcome is unknown",
        ),
        E::Insufficient => {
            ControlError::new(Code::Insufficient, "insufficient credits or spending bound")
        }
        E::Denied => ControlError::new(Code::Denied, "the peer did not authorize the request"),
        E::Limit => ControlError::new(Code::Limit, "a finite limit was reached"),
        E::Unsupported => {
            ControlError::new(Code::Unsupported, "the peer does not support the request")
        }
        E::Storage | E::Invalid | E::Conflict | E::EvidenceUnavailable => ControlError::new(
            Code::Refused,
            "the peer refused the request before any commitment",
        ),
        _ => ControlError::new(Code::Unavailable, "the peer exchange was unavailable"),
    }
}

// Build a typed logical object without numeric coercion.
fn fields<const N: usize>(members: [(&str, Typed); N]) -> Typed {
    tracing::trace!("Encoding typed Storage fields");
    Typed::Object(
        members
            .into_iter()
            .map(|(name, value)| (name.into(), value))
            .collect(),
    )
}
