# Placement lifecycle

Every change goes through `Store::update`, which applies one closure to one record under the store lock, so conflicting operations on a placement are serialized and exactly one definite ordering results. Expiration is lazy and uses the
provider clock; an operation uses the time at which it was accepted.

- `touch(store, id, now)`: a pending (or recovered) placement whose expiry was reached at `now` becomes `Expired`, recorded once; the record is returned.
- `mark_recovered(store, id, now, receipt)`: only a pending, unexpired placement; records the receipt with the move to `Recovered`. Otherwise `NotAvailable`.
- `mark_removed(store, id, now, expected_expires_at, removal)`: only a pending, unexpired placement; a wrong expectation is `Conflict`; records the removal result with the move to `Removed`.
- `extend(store, id, now, expected, additional_seconds, ceiling, price)`: an active (pending or recovered) placement, the exact expected expiry (`Conflict` otherwise), a new expiry of `expected + additional` not past `ceiling` and
  payments plus `price`, all overflow-checked (`Limit`); the state does not change.
- Allowed transitions: `receiving -> pending -> recovered | removed | expired`; terminal states never move. Recover and remove are mutually exclusive; the expiry instant itself is expired (`now >= expiresAt`).

Evidence: boundary tests at the expiry instant, finality, stale and overflowing extensions, and threaded races (eight threads racing recover against remove, six racing renewals from one expectation) with exactly one winner and a consistent final state.
