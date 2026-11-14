# Wanted endpoint orchestration

`Publisher` is explicitly installed local application authority, distinct from a
wanted Board entry. Its `policy` supplies bounded local input/spending for the
exact current wanted contract and authenticated prospective provider. Its `consent`
separately approves an independently validated immutable quote. Its capacity-bounded
`completed` sender observes final original-scope results without blocking workers.
Absent policy is Unsupported; a peer invitation or published document never creates
input, installs code or grants spending. Applications should keep work selection
bounded and refuse when no locally authorized work item is available.

`Invitation` supplies only a sealed wanted selection and its directly authenticated
metadata cache. It supplies no fabricated requester input, payment permission or
installation hints. A prospective provider submits it through the one-shot
`Plan.Offer` factory on the actual publisher's authenticated connection. Installed
provider semantics and current direct publication are required before sending an
offer. The opaque original invitation binds any publisher proposal to its exact
owner, stream, roles and captured contract.

The publisher checks its authoritative current wanted Board before and after local
policy selection. Only then does it become the requester, send a locally authorized
proposal, independently validate the quote and reserve through its own coordinator.
The provider still issues/accepts exact terms, waits for complete input and requester
END, commits local participants and sends only committed output. Publisher result
validation/commit waits for actual normal completion. Opener direction never selects
the requester, payer, issuer or financial key.

Both sides expose bounded typed completion observations identifying their original
local operation. Dropped/full observers cannot undo durable effects or trigger a
retry. Normal explicit refusals retain their failure category. Interrupted accepted
work remains unknown in the original owners. Included evidence reconciles local END
submission observations conservatively; no new acknowledgement or refund is added.

See [ordinary invocation](requests/invocation.md), the
[generic wanted contracts](../../wanted.md) and [request factories](requests/mod.md).
