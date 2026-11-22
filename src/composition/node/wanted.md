# Wanted Storage contributions

When `credit.wanted` is set in [policy.json](../policy/params.md), this node publishes a wanted Storage offering
(`storage-wanted`) and pays clients in its own credits for holding its data. It is the requester; the client is the
provider.

- **Terms**: `creditsPerObject` is both the published minimum price and the exact spending bound for one object;
  `maximumObjectBytes` and `retentionSeconds` bound the object. The description matches the provided offering because it is
  part of the contract identity the installed Storage family recognizes.
- **Work source**: `Work` is the publisher policy. Hosted items from the [replica book](replicas/mod.md) that still need copies
  are offered first, then `wanted-add` queues objects (at most 64, nonempty, within the terms). When a
  client offers to fulfil the terms, `Work` chooses the next queued object and the spending bound; the client's invitation
  selects neither. A client has at most one object in flight; work that never completes is dropped after two minutes as
  lost and is not re-queued.
- **Completion**: a handler thread validates each committed store against the original input, catalogs the lease as one of this
  node's replicas (contact: the client's ID, reachable over its own connection) and records a verified contribution for
  the trust policy. A definite failure returns the object to the backlog; an unknown outcome is logged and never retried.
- **Credits**: the client earns this node's credits, bounded by the Ledger issuance budget (`credits.issuance`). The client
  must trust this node before offering, because a provider only works for requesters it trusts.
- **Commands**: `node-ctl wanted-add --file F` on the publisher; `node-ctl fulfil --contact C` on the client. `status` shows
  the queued and in-flight counts.
