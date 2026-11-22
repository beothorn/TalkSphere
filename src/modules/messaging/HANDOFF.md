# Messaging handoff to node composition and the client journey

Upstream behavior: [CR2SE 73edf2d](https://github.com/beothorn/CR2SE/tree/73edf2dc230bccb0f27f2ea6e06e938fff738862) (issue
[beothorn/CR2SE#13](https://github.com/beothorn/CR2SE/issues/13) is open for the Peer Protocol `resource_limit` text). The module is
complete at the engine, Services and Peer Protocol levels; the pieces below are what a node must fill in and what is still missing.

## What a provider node composes

Order: open the Messaging store and journal, the Ledger and the Services journal; build the participant with a
[`LedgerIssuer`](service/release.md) and recover it with `DeferToServices`; build the engine with the two ports below; build
`ServiceProvider` and `Client`; register both with the registry; open the coordinator; call
[`service::recovery::resume`](service/recovery.md); only then publish the offering. A `ServiceProvider` refuses to be built before the
participant recovered, and the Board refuses the offering while no handler is registered.

| Port | Fill it from | Notes |
| --- | --- | --- |
| `ReleasePolicy` | peer policy's `Credit::release(cumulative_payments, minimum)` | A closure `Fn(u64, u64) -> u64` is already a policy. The module bounds the answer by the placement's payments and by the sender's minimum. |
| `PendingLimits` | the policy's per-identity limits | Per sender and per recipient pending placements; refusal happens before any body or charge. Request and failure rates stay in peer policy. |
| `ReleaseIssuer` | `LedgerIssuer(ledger, Coordinate grant, timeout)` | Needs the Ledger's once-per-operation `issue_release`; issuance is idempotent per operation key. |
| `Signer` | the node's protected Identity `sign` with a `Sign` capability | Receipts are signed by the provider identity; senders and recipients sign messages and recovery requests the same way. |
| `Clock`, `Entropy` | the node's native clock and system entropy | Times are Unix seconds. |
| `Placements` (client) | the requester's catalog | The payload size a requester stored or discovered; a provider can never choose the size it is paid for. |

## Peer policy integration

- **Check outcomes.** `Client` (as `Rules`) turns a paid byte check into `Check::Pass`, `Fail` or `Inconclusive` from the sender's private
  expectation `{placementId, offset, expectedByte}` and the trusted timing. Composition should report `Pass` and `Fail` of a *timely*
  check to `Owners::observe(provider, Event::CheckPassed | Event::CheckFailed)`; `Inconclusive` is never a failure and should not be
  observed.
- **Redundancy.** A sender's message placements at distinct providers are replicas of one item keyed by `(senderId, messageId)`: the
  same signed message stored at several providers. A replica counts toward the target only after the store's committed result
  (acceptance and expiry times), expires at that placement's `expiresAt` (renewals extend it) and stops counting when the provider's
  status or check fails. Distinct providers are what makes copies independent, never two placements at one provider. A recipient
  recovers from any replica and `receive::Inbox` delivers the message once.
- **Release credits.** Removing the copies a sender no longer needs releases credits once per removal; the recorded removal answers an
  exact retry for the ledger's lifetime.

## Client side (client journey, plan 14)

- `wire::Remote` builds the endpoint plans with exact ceilings; `receive::Inbox` authenticates, deduplicates by `(senderId, messageId)`
  and decrypts (its table is in memory: the client catalog must persist it, plan 14 step 6).
- **Identity.** Identity gained a scoped `agree` operation (guarded by `Decrypt`) and `messaging::encryption::decrypt` now takes a
  `PersistentAgreement` holder; [`IdentityAgreement`](identity.md) adapts a loaded Identity, and
  [`composition::messaging::NodeIdentity`](../../composition/messaging.md) adapts a node's shared protected identity (the same one
  Transport authenticates with) as signer and agreement holder, with `SystemClock` and `SystemRandom` for the other ports. The
  Board `identityEncryption` credential is checked against the independent vectors; publishing it is the client journey's job.
- **Placements port, both roles.** A requester's `Placements` is its own catalog; a provider installs
  [`StorePlacements`](service/placements.md) so its own `Client` re-prices recoveries and renewals from its durable records and
  discloses sizes only to a placement's sender or recipient.

## Known limits

- Removal tombstones are kept for the ledger's lifetime and count toward the placement bound.
- Incomplete uploads are held in memory under a bounded reservation and are not durable; the staging reservation taken while assessing a
  store is released at once because the body has not arrived (execution re-validates in full).
- A read-only operation (`discover`, `status`) whose staged result is lost is produced again after a restart rather than replayed.
- The exercise ([guide](../../../docs/messaging/EXERCISE.md)) runs four authenticated nodes in one process, each with its own
  protected identity, listener and native stores, over real loopback TCP. It does not restart a provider or run one in a separate
  process (Storage's example does); crash and reopen are covered by `service::crash` instead. Its offering widens the pinned
  fixture's 100-byte payload bound, which is smaller than an encrypted payload.
