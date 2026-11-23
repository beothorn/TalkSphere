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

**List.** `list(reader, request)` returns one publisher's active placements of the asked kind, newest first by `(sequence, publicationId)`
descending, strictly after the cursor, at most `limit` (`1 ..= maximumListResults`), with `more` true only when another readable entry followed,
and the fixed list price. A reader may read public publications, restricted ones naming it in the audience, and its own; the rest are omitted
without any difference in entries, `more` or price. An empty page is a successful charged list. A resource kind or limit outside the bounds is
`Invalid`; no clock is `Unavailable`. Expired and removed placements are not listed.

**Get.** `get(reader, request)` returns the complete active placement of the intended publisher's publication when the reader may read it, with
price `max(minimum, ceil(contentSize * R))`. Unknown, wrong-publisher, expired, removed and unauthorized publications are the same `NotAvailable`;
a `maximumBytes` of zero or above the offering is `Invalid`; content larger than the reader's bound is `Limit`; content the host cannot return
completely is `Storage`. The reader verifies the bytes itself. Repeating a get is a new invocation and charge.

**Renew (publisher only).** `assessRenew(publisher, request)` requires a pending, unexpired placement of the publisher (otherwise one concealed
`NotAvailable`), added seconds within the offering's retention limits (`Invalid`), the exact expected expiry (`Conflict`, checked here so a used
expectation is never mistaken for the repetition of an applied renewal) and `expected + additional` not past `acceptedAt + maximumTotalRetentionSeconds`
(`Limit`); the price is `max(minimum, ceil(size * seconds * N))`. `commitRenew(plan, at)` extends the expiry from the expectation and adds the price to
the payments; the expiry only grows, so a later expiry means the renewal was already applied and the output is rebuilt (crash repair).

**Remove (publisher only).** `quoteRemoval` fixes the release credits: the policy's offer bounded above by the placement's cumulative store and renewal
payments (list, get, remove and check prices are never added) and below by the publisher's minimum (`Invalid` when the minimum exceeds the payments,
`Limit` when the policy offers less, `Conflict` for a stale expiry), with the remove price. `assessRemove` answers an exact retry (same removal ID on
the removed placement) from the durable record, with no expiry check; otherwise it quotes. `commitRemove(key, request, quote, at)` checks state and time,
issues the credits once under the Services operation key, records the removal and releases the content; repeating it after the removal was recorded
issues again idempotently, so an interrupted settlement completes exactly once. Credits without an installed issuer are `Unsupported`; a refused
issuance leaves the placement pending. A different removal ID on a removed placement is `NotAvailable`.

**Check (publisher only).** `check(publisher, request)` returns the content byte at `offset` (a ciphertext byte for restricted content) of a pending,
unexpired placement; an offset at or beyond the size is `Invalid`; foreign, unknown, inactive and expired placements are one concealed `NotAvailable`.

The one-shot `renew`, `remove` and `storePublication` serialize assessment and commit with an internal lock; a caller that separates the two steps
(the Services participant) must lock the placement between them.

**Header assessment and sizes.** `assessHeader(publisher, metadata, retention)` judges a store from its header alone (authenticated publisher, metadata rules, a restricted size that can hold its envelope, retention,
room, quota) and returns the exact price; `assessStore` repeats it after the content arrived and adds the signature, size, hash and envelope checks. `readableContentSize(reader, publisher, publication)` gives the size a get
would return, concealed like get itself, so the price can be stated before the work.
