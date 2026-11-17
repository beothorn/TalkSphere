# Timely retrieval acceptance

accept consumes trusted accepted Invocation, complete input, owner Image,
protected byte Store, coherent current Time, optional trusted reboot elapsed
interval and independent Value Budget. Services must already have authorized the
quote/reservation and complete input; owner admission bounds concurrent pins.

It requires the actual provider, requester/key, provided-offering publisher and
issuer associations. The accepted offering supplies current Storage terms;
original store terms are preserved on the lease. Missing/foreign resources both
fail Denied before protected metadata is inspected. Expired/removed/receiving
leases fail Unavailable. Corrected monotonic retention time determines timely
acceptance; unverifiable reboot intervals remain unavailable.

Positive range validation uses offset < size and length <= size - offset, plus
the accepted current retrieval limit. Exact price is independently recalculated
and compared with fixed accepted pricing/maximum. No extra provider factors are
permitted. Byte reopen/length failures never fabricate a successful pin.

Pinned retains the original lease snapshot, zero-based object offset, exact work
price and a scoped Range. bytes exposes only that accepted subrange; lease includes
the whole-object hash. Later metadata/Board changes, expiration, removal or unlink
do not invalidate its retained bytes. Result creation and durable service staging
remain separate operations; this helper neither sends data nor charges credits.

This independent range contract does not resolve upstream issue #11 or register
a paid Storage provider. Caller composition must persist authoritative terminal
observations and bound ownership through existing owner and Services contracts.

Pinned.key retains the original trusted operation association. Exact accepted
terms are also retained privately for local result binding; no new peer replay
capability or filesystem path is exposed.
