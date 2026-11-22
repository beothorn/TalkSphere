# Messaging prices

Exact integer arithmetic over rates `numerator/denominator` (`Rate`); floating point is never used. `minimum` is a floor, not an extra fee.

- `store(size, seconds, upload, retention, minimum)` = `max(minimum, ceil(size*B + size*seconds*X))`: both exact terms are added before one upward rounding.
- `recover(size, rate, minimum)` = `max(minimum, ceil(size*R))`.
- `renew(size, additional_seconds, rate, minimum)` = `max(minimum, ceil(size*additional_seconds*N))`.
- Discover, status, remove and check prices are the offering's fixed values; they are validated with the terms, not calculated here.

Zero size, duration, minimum or denominator is `Invalid`; a result above uint64 is `Limit`. Acceptance-harness values: `store(3, 10, 1/2, 1/2, 1) = 17`,
`store(1, 1, 1/2, 1/2, 1) = 1`, `recover(3, 1/2, 1) = 2`, `renew(3, 4, 1/2, 1) = 6`.
