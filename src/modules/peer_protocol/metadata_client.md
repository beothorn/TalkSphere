# Original-stream metadata consumption

Request.board takes local Request authority, its owner domain and an Association.
Association holds local identity, authenticated peer identity and exact operation
key (unique connection instance plus positive stream); these are trusted adapter
observations, never payload assertions. The metadata requester must be local.
Request.definition additionally pins the selected directly attributed Remote
receipt and exact offering ID; forwarded/wrong-publisher receipts are denied.
Wire returns the corresponding request fields without opening connections.

Apply consumes a complete normally ended metadata response. It separately requires
Invoke authority for trusted incoming events, checks the original complete
association and original deadline, and validates expected response kind. The
Board cache supplies its separate publisher-bound Cache grant. Exact raw Board/
definition subdocument bytes are passed through public store/attach contracts,
without JSON reserialization or binary64 conversion. The direct publisher is the
actual authenticated opposite endpoint; document fields confer no provenance.
Stale/missing/current-receipt, exact definition/version and cache limits are
independently checked by Board. Changed connection/stream/peer, forwarded pins,
revocation or late delivery cannot install/replace a receipt. No hidden refresh
or nearby-version substitution occurs. The owning engine must require normal
exchange completion before apply; this local helper is not Transport authentication.

Board errors preserve invalid, denied, limit, cancelled, unsupported, storage and
unknown categories; Stale maps to Conflict and Missing to Unavailable. Received
wire failure messages follow the separate exchange failure path, never cache
installation. There are no Services, payment or key effects.
