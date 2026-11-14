# Direct peer metadata cache

`PeerCache.new` creates an empty volatile bounded cache over an independently
installed Registry and Board limits. It performs no network fetch or publication.
The wrapper retains its own cache authority; peer JSON cannot populate it or
choose attribution. Normal original-stream request completion uses the existing
metadata-client gate to store exact Board bytes or attach a validated definition.

`selection(publisher, offering, service, version)` selects only a fresh direct
receipt with an attached definition. Expected description and semantics come from
the locally installed requester contract. Missing, expired, forwarded, unsupported
or changed contracts fail explicitly. The opposite participant is the Registry's
local identity; direction and issuer derive from validated offering terms.

Cache age uses checked process-local monotonic whole seconds and the configured
Board expiry policy. A restart starts with an empty cache. Immutable receipts do
not stay fresh forever, and attached definitions do not invent a new publisher.
No hidden fetch, global clock, mutable cache table or authority issuer is exposed.

Outgoing acceptance rechecks current cache freshness, installed definition,
participant roles, issuer and direction. Quote economic differences still pass
independent pricing validation, original-request bounds and separate exact-snapshot
local consent; copied metadata alone is never spending permission. This cache is
not an authoritative local Board owner or a forwarded settlement-receipt verifier.
