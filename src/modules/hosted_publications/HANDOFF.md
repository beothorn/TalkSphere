# Hosted Publications and Static Pages handoff to feeds, node composition and the client journey

Upstream behavior: the CR2SE Hosted Publications proposal ([CR2SE 53f1245](https://github.com/beothorn/CR2SE/tree/53f12451859e0ad207ade9b8a23be20ceb404495), proposed in
[beothorn/CR2SE#14](https://github.com/beothorn/CR2SE/issues/14), **not accepted upstream**) over [CR2SE 73edf2d](https://github.com/beothorn/CR2SE/tree/73edf2dc230bccb0f27f2ea6e06e938fff738862) for Static Pages. Both modules are
complete at the engine, Services and Peer Protocol levels (see their acceptance records); the pieces below are what a node must fill in and what is still missing.

## What a host node composes

Order: open the placement store and journal, the Ledger and the Services journal; build the engine with the ports below and a `LedgerIssuer` for release credits; build the participant and recover it with `DeferToServices`;
build `ServiceProvider` and `Client` (with `StorePlacements(store)` as the placement-size port); register both; open the coordinator; call [`service::recovery::resume`](service/recovery.md); only then publish the offering.
`ServiceProvider` refuses to be built before the participant recovered. `examples/pages/owners.rs` is a working composition over native stores.

| Port | Fill it from | Notes |
| --- | --- | --- |
| `ReleasePolicy` | peer policy's `Credit::release(cumulative_payments, minimum)` | Reused from Messaging; the module bounds the answer by the placement's payments and by the publisher's minimum. |
| `PendingLimits` | the policy's per-identity limits | Reused from Messaging: `per_sender` bounds a publisher's active placements at the host. |
| `ReleaseIssuer` | `messaging::service::release::LedgerIssuer(ledger, Coordinate grant, timeout)` | Idempotent per operation key. |
| `Signer` | the node's protected Identity (`composition::messaging::NodeIdentity`) | Publishers sign publications with it; the same identity opens restricted publications through `PersistentAgreement`. |
| `Clock`, `Entropy` | `SystemClock`, `SystemRandom` | Times are Unix seconds. |
| `Placements` (client) | a publisher's catalog on the requester side, `StorePlacements` on a host | A host reads a renewal's size from its own store. |

## Client side (feeds, client journey)

- `wire::Remote` builds the endpoint plans with exact ceilings. A reader lists, gets (bounded by the size the list entry states), verifies with `verify` and, for restricted content, opens with `envelope::open`.
- `pages::hosted::build` and `assemble` create and read page snapshots; `latest::select_latest` picks the best-known version across hosts and reports equivocation. Both are shared with feeds: a feed post is a
  `feed-post` publication whose content is Public Feeds canonical message bytes.
- The publisher's own durable catalog of what it stored where (placement IDs, expiries, sizes) is the client journey's to keep; the exercise keeps it in memory.

## Peer policy integration

- **Redundancy.** A publisher's placements at distinct hosts are replicas of one publication keyed by `(publisherId, publicationId)`: the same signed bytes at several hosts. A replica counts only after the store's committed result,
  expires at that placement's `expiresAt` (renewals extend it) and stops counting when the host's check fails. Pages keep each resource at least as long as any retained manifest naming it.
- **Check outcomes.** `Client` (as `Rules`) turns a paid byte check into `Pass`, `Fail` or `Inconclusive` from the publisher's private expectation `{placementId, offset, expectedByte}`; only a timely pass or fail is reported to peer
  policy, and `Inconclusive` is never a failure.
- **Release credits.** Removing old versions releases credits once per removal.

## Still missing

- **Milestone 16 (Public Feeds):** the feed-post content checks (HP-09), feed merging and the publisher-served `cr2se.open-social-messaging`.
- **Node composition and the client journey (plan 14):** installing the hosted publication and Page families in `serve`, publishing their offerings after recovery, and publisher and reader commands over a node.
- **Upstream:** acceptance of the proposal, and the three open questions in [REQUIREMENTS.md](REQUIREMENTS.md): HP-05's equal-sequence seed, the sizing of restricted manifest entries, and the Peer Protocol `contentSize` pricing input
  (a local CR2SE commit, not pushed).
