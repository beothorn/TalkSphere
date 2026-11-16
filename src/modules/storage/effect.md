# Exact Storage metadata proposals

Effect is NoChange, Create(Lease), Renew(expected expiration, proposed Lease), or
Remove(ID, expected expiration). These are local owner proposals, not new peer
fields. Byte objects and complete results must already be staged by their owners.

encode/decode preserve exact proposal bytes under image limits. Proposed lease
records use this owner's public versioned image codec, with one lease and no
unrelated operations. Decoding binds the expected provider and rejects malformed,
truncated, trailing, oversized or future data. resource identifies the lease
requiring exclusive mutation preparation; NoChange has no mutation target.

apply changes only a private candidate Image under authenticated requester
identity. Create rejects reused IDs and foreign ownership. Renew compares the
original expiration and preserves ID, requester/provider, size/hash, original
terms, acceptance, store price and active state while increasing expiration.
Remove serializes against the observed expiration and marks only active owned
leases Removed. Missing and foreign targets both return Denied; guard/state/ID
conflicts return Conflict. Advertised terms, acceptance timing, exact prices and
content validity must already have been independently checked by the handler.

The journal may persist this candidate only after receiving the coordinator's
immutable commit decision. Neither constructing nor applying an Effect authorizes
payment, creates a remote retry key or proves remote receipt. Renewal/removal
proposals cannot rewrite original byte or economic obligations.
