# Original-key retry ownership

cached_provider requires Observe authority, authenticated new Peer and decoded
Proposal. Only Messaging version one with an explicitly approved recovery/removal
identity is eligible. It scans bounded retained owner records and pins without
active Registry selection, current offering prices or Ledger balance checks.
Placement/purpose/ID, original caller, every defined input field, offering,
publisher, issuer, spending bound and transfer tuple must match. Conflicting
reuse fails; unrelated scopes reveal no cached output. Matching uncommitted work
remains Unknown. A committed match still requires the family's explicit cached
ownership/retention approval before returning opaque Cached proof.

retry_requester requires separate Request and Observe contexts, original durable
key and a different authenticated provider stream. It permits only an original
already dispatched eligible Messaging operation, never ordinary work retry, new
acceptance or fresh spending. The opaque Retry belongs to this exact coordinator
instance; getters preserve original input, quote and payment bounds.

receive_replay requires Coordinate authority, that exact Retry and observed new
Peer. It rechecks original input/family identity and remaps correlation only inside
the original owner, then applies normal complete-result validation to the original
operation. Original quoted schema/factors/price remain authoritative. Duplicate
identical results use existing journal and Ledger idempotence; contradictory
results fail. No adapter pretends a new Transport stream was the old one. Retry
failure cannot undo the original possible commitment.

Family callbacks must be bounded and must not reenter this coordinator while its
serialized state is held. Missing capabilities never fabricate a cached result.
