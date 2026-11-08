# Exact quote binding and acceptance

Quoter owns one local identity/Authority, finite Limits, an opaque domain and a
bounded transient quote map. Issue contains an Assessed selection, complete exact
quote bytes from the trusted Peer Protocol encoder, explicit requester maximum
and monotonic expiry. The adapter must bind those bytes to the assessed economic
fields; this module does not parse or redefine the peer wire format.

issue checks authority, bytes, positivity/pricing/ceiling, duplicate stream,
expiry and quote capacity. It returns an opaque Quote exposing only exact bytes
and SHA256 hash. Expired unaccepted slots are reclaimed. Neither quoting nor the
returned handle reserves credits or executes work.

accept checks original owner domain, local/remote peer, requester/connection/stream,
exact quote hash and expiry. It calls Current.check and rechecks authority before
consuming the slot once. BoardCurrent adapts the public Board owner/current-selection
contract using a valid Board Select context. Changed or removed terms conflict;
uncertain Board state remains Unknown. No replacement quote is silently accepted.
Current implementations must be bounded local checks and must not reenter Quoter.

Accepted retains assessment, exact bytes, pricing and requester ceiling for the
coordinator. It records explicit negotiation consent; successful execution still
requires durable operation admission and Ledger reservation. A duplicate local
acceptance has no second slot; later durable retries use the original operation
key, not a fresh quote. disconnect releases only unaccepted connection slots
under trusted invocation authority. Accepted work has independent ownership.
