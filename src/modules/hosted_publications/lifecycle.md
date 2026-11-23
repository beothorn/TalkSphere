# Hosted Publications lifecycle

Every change goes through `Store::update`, which applies one closure to one record under the store lock, so conflicting operations on a placement
are serialized and exactly one definite ordering results. Expiration is lazy and uses the host clock; an operation uses the time at which it was accepted.

- `touch(store, id, now)`: a pending placement whose expiry was reached at `now` becomes `Expired`, recorded once; the record is returned.
- `mark_removed(store, id, now, expectedExpiresAt, removal)`: only a pending, unexpired placement; a wrong expectation is `Conflict`; records the
  removal with the move to `Removed`.
- `extend(store, id, now, expected, additionalSeconds, ceiling, price)`: a pending unexpired placement, the exact expected expiry (`Conflict`
  otherwise), a new expiry of `expected + additional` not past `ceiling` and payments plus `price`, all overflow-checked (`Limit`); the state does
  not change. Only the first of several renewals from one expectation wins.
