# Independent original-issuer settlement

Ports supplies the original local requester/provider Ledger, its authority and
separate Evidence/Coordinate contexts. These grants are local composition policy,
never decoded from peer input. Work completion and payment resolution are distinct.

next requires Peer Protocol Coordinate permission and an original locally owned
invocation association. Ledger returns the exact retained submit/query action only
for committed unresolved third-party work: original issuer, owner/newOwner, transfer
ID, gross and fee. A requester signs/submits that original order; a provider uses
query after the original invocation exchange has ended normally. query requires
outgoing Request permission, provider query role, correct issuer and a different
connection from the invocation. It never asks the requester to forward a receipt.
Composition must derive all associations and normal termination from Transport.

confirm accepts only an opaque normally completed issuer Transfer observation. It
checks local owner and independent connection, rejects requester/local-node sources,
binds the actual authenticated issuer under Evidence authority and delegates exact
owner/newOwner/ID/gross/fee/net validation to Ledger's original operation. Confirmation
is idempotent and lifetime transfer bindings prevent another invocation reusing the
payment. No new service execution, quote, hold or changed fee is accepted.

problem records Unreachable as Unknown and explicit Refused/FeeChanged as Unsettled,
retaining obligations. It cannot alter the already committed service effect or
reverse settled payment. Correct original evidence may resolve an unsettled attempt
later; no automatic retry, compensation, refund or silent fee substitution occurs.

Tests use real three-party books, strict signed issuer transfers and complete
protected query replies. The financial participant fixtures do not by themselves
claim real-process service/network evidence; those are separate integration steps.
