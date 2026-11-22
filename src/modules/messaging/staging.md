# Staging

A bounded accounting of payload bytes and uploads that are being received but are not yet durable.

- `Staging(max_bytes, max_count)`: a shared budget; clones share the same counters.
- `reserve(bytes)`: takes a reservation of `bytes` and one upload slot. Fails with `Limit` when either budget would be exceeded; nothing is taken on failure.
- `used()`: the bytes and uploads currently reserved.
- A `Reservation` releases its bytes and slot when dropped, on success, failure or abandonment.
