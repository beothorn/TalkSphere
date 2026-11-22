# CR2SE quota and refusal review

Review of how [CR2SE bc53df4](https://github.com/beothorn/CR2SE/tree/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16)
lets a provider refuse work for a requester's identity, written while implementing
[peer policy](../../plans/12_Peer_policy/PLAN.md) step 6.

## What CR2SE already says

- Providers apply per-sender and per-requester quotas, rate limits and sender trust by local policy
  ([Messaging security requirements](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/modules/messaging/README.md#18-security-and-interoperability-requirements),
  [Storage rate limits](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/modules/storage/README.md)).
- A refusal before acceptance is uncharged and uses the `resource_limit` failure code
  ([Peer Protocol failures](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/modules/peer-protocol/README.md)).
- Metadata exchanges are bounded, rate-limited control traffic
  ([Ledger trust and bootstrap](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/modules/ledger/README.md#trust-and-bootstrap)).

## Gaps found

1. **A requester cannot tell a temporary refusal from a permanent limit.** `resource_limit` covers size,
   concurrency, time and capacity, and says nothing about whether or when to retry. A scheduler that must
   keep many replicas fresh, or a phone on a flaky connection, can only guess or never retry.
2. **A rate limit has no specified refusal.** The only specified way to stop a flooding identity is to close
   the connection, which ends every exchange on it. Exchanges already accepted then have an unknown outcome
   although the provider only meant to slow the peer down. TalkSphere currently closes the connection for
   exactly this reason: its stream layer has no per-identity reply hook.
3. **Per-requester limits cannot be advertised.** Offering `info` carries per-operation maxima but nothing
   about per-identity concurrency or rates, so requesters cannot choose providers by what they will allow.
   Not proposed yet: it needs real usage data first (see the simulations in step 20).

## Proposal (gaps 1 and 2)

An optional `retryAfterSeconds` (whole seconds, 1 to 86400) on a `failure` with code `resource_limit`, sent
before acceptance. A requester must not repeat the request earlier and may retry automatically once the time
passes; without the field there is no automatic retry, and the unknown-outcome rules are unchanged. Providers
should refuse new exchanges of a rate-limited identity with this failure and keep closing for exhausted
resources or continued abuse. Language-independent text, acceptance case PP-09A and a design decision are
committed in the CR2SE repository (commit `73edf2d` of the local checkout, not pushed).

## Status

- CR2SE change: pushed as [73edf2d](https://github.com/beothorn/CR2SE/commit/73edf2dc230bccb0f27f2ea6e06e938fff738862) and reported as
  [beothorn/CR2SE#13](https://github.com/beothorn/CR2SE/issues/13) on 2026-11-22. It is a design proposal rather than a confirmed
  defect; no earlier issue covered it (all nine existing issues were checked).
- TalkSphere: unchanged. It will adopt the failure message, a requester back-off and rate refusals instead of connection close only after
  the change is accepted; until then its documented behavior is the connection close described in
  [the runtime guide](../../src/composition/node/runtime.md).
