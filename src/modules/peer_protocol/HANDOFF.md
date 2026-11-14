# Storage and minimum-node handoff

Peer Protocol's generic wire/owner integration is complete under the scopes in
[its acceptance matrix](ACCEPTANCE.md). Paid Storage remains the next required
module on the [roadmap](../../../plans/PLAN.md); this document does not implement
or expand a new detailed Storage plan. The persistent minimum useful node and
optional families remain separate milestones.

The baseline is
[CR2SE d40b6278f24430e814d7fa82094f57261bfcac9f](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f),
rechecked2026-11-14 with no newer protocol change. Before planning Storage,
review its [specification](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/README.md),
[inputs](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/inputs/README.md),
[outputs](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/outputs/README.md),
[pseudocode](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/pseudocode/README.md),
[acceptance cases](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/acceptanceTests/README.md)
and shared composition/recovery contracts. Record the revision used by its own
full-definition, formula and cryptographic fixtures.

## Existing integration contracts

- Register actual Storage requester/provider implementations through Services Registry with the complete pinned standard definition and an independent Standards catalogue; remote definitions cannot install code or silently authorize a new version.
- Publish actual provided/wanted offerings through Board, with exact issuer/pricing/preconditions; direct metadata cache and sealed selection retain authenticated publisher provenance and current-term checks.
- Use peer request plans and separately installed consent for locally chosen input and explicit total spending bounds; no peer invitation chooses requester data, encryption or payment policy.
- Deliver validated unresolved descriptors to assessment without placeholder bytes, then complete exact input only after acceptance, ordered bodies, input.end and requester END; dispatch and lease acceptance remain distinct durable owner transitions.
- Retain Storage effects, preparation tokens and exact output through independent durable participant state before acknowledging commitment; Peer Protocol sends only committed output and never reexecutes a family to regenerate a lost reply.
- Validate received content/hash, requested range, size, price/finalPrice and operation-specific guards independently before requester commitment; peer framing provides no application payload confidentiality.
- Keep local module persistence separate from the paid Storage family; one lease is one provider copy, with redundancy/replication remaining application work.

## Storage responsibilities still to implement

Storage owns immutable byte leases and exact authorization, size/hash, accepted
offering snapshot, start/expiry and operation guards. Implement store, ranged
retrieve, renewal, early removal and charged byte checks according to its current
contract. Add complete standard pricing/formula and provider-reported renewal
`sizeBytes` consistency cases; Peer Protocol currently verifies their exact wire
projection and calls installed independent family logic, not a Storage backend.

Build durable staged bytes and lease metadata, finite quotas, participant
prepare/status/commit/abort and crash recovery before advertising the service.
Exercise interrupted upload, committed reply loss, expired/foreign/missing leases,
range boundaries, changed terms, hash mismatch, renewal/removal conflicts and
concurrent quota/spending decisions. Preserve original unknown effects; ordinary
Storage work has no generic remote resume/status or automatic cached replay.
Family-specific retries require an explicitly defined and validated contract.

A remote check must accept only the public challenge, use the selected exact
resource and return specified evidence without a provider-computed correctness
outcome. Private expected data and pass/fail/inconclusive remain requester-local.
Bind response timing to original Transport input/output/closure observations,
which already survive worker delay. A paid check cannot infer a provider refund
from local timeout; the unresolved
[upstream issue10](https://github.com/beothorn/CR2SE/issues/10) still limits global
no-charge claims after provider commitment and response loss.

Third-party work and payment remain separate durable outcomes. Use preagreed
transfer ID/exact fee inside the total bound; owner signing alone is not fee
consent. A provider must obtain an opaque normally completed observation on its
own independent issuer connection, then reconcile exact original owner/new-owner,
ID and gross/fee/net once. Refused/unreachable/changed-fee payment does not reverse
already committed Storage work or fabricate a refund/trust adjustment.

## Evidence to extend

The [runtime example](../../../docs/peer-protocol/RUNTIME.md) demonstrates actual
authentication and custom inline work using volatile service/accounting resources;
it is not a persistent paid Storage node. [Node tests](../../../docs/peer-protocol/NODES.md)
add detached values, wanted roles, paid/included evidence, complete three-party
settlement and actual issuer process restart. Extend those patterns with real
persistent Storage byte/lease resources and independently validated family
fixtures. Keep generic wire tests, complete standard family cases, real process
recovery and cross-implementation interoperability claims distinct.

The minimum useful node milestone must compose all eight required modules with
authorized local control and configured contacts, complete a paid nonempty store
and verified retrieval across restart, then renewal/check/removal with finite
credits and concurrent failures while optional modules are absent. Default
`serve` remains Transport-only until an explicit complete operator profile is
implemented and documented. No further module is implemented by this handoff.
