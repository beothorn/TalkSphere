# Hosted Publications provider

`Provider::new(terms, store, clock, entropy, limits)` is the engine over one placement store; `limits.per_sender` bounds one publisher's active
placements. Each operation is an `assess` step that checks every rule and prices the work without changing anything, then a `commit` step that
applies it durably at a decision time, so Services can quote first and apply once. Failures before the commit change nothing and are uncharged.

**Store.** `assessStore(publisher, request)` requires: the publication verifies against the authenticated publisher within the offering's audience and
content limits; a restricted publication has an envelope whose entry count equals the audience; the retention is within the offering's minimum,
maximum and total limits and `now + retention` does not overflow; the store has room and the publisher is under its quota. It returns a plan
with the exact price (`Invalid`, `Limit` or `Unavailable` on failure). `commitStore(plan, placementId, at)` records a pending placement with
`acceptedAt = at`, `expiresAt = at + retention` and `payments = price`, content first and then the record; a duplicate publication ID or a sequence
already used by another active page or feed post of the publisher is `Conflict`; repeating the same commit returns the recorded output (crash
repair); the same placement ID for a different publication is `Conflict`. `storePublication` does both with a fresh unpredictable placement ID.
`newPlacementId` fails with `Unavailable` without entropy and never invents an identifier.
