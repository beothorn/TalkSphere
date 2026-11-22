# Provider

The provider engine for the store operation. It never charges or records anything before the final commit.

- `StoreHeader`: the signed metadata, sender key, sender signature and requested retention, i.e. everything except the payload bytes. `StoreInput.header()` extracts it.
- `Provider(terms, participant, clock, staging)`.
- `assess_store(caller, header)`: before the body is received, checks that the sender is the authenticated caller and derives from the supplied key, the payload size is between 1 and the offering maximum (at least 105 for the encrypted encoding), retention is within the offering bounds, the expiry does not overflow, and calculates the exact price. Reserves staging space. Returns an assessment holding the price and reservation, or `Invalid`/`Limit` with nothing reserved.
- `prepare_store(key, assessment, message)`: requires the message to equal the assessed header, verifies size, hash and signature (the signature covers the payload, so this can only happen now), then prepares the effect durably with a new placement ID.
- `commit_store(prepared)`: commits and returns placement ID, message ID, acceptance time, expiry and price. The acceptance time is assigned at the durable commit.
- `store(key, caller, input)`: the three steps in one call. Dropping the assessment releases the reservation.
- Every failure before commit leaves no placement, no journal entry and no reservation.
- `discover(caller, input)`: lists the caller's pending, unexpired placements ordered by `(acceptedAt, placementId)` strictly after the optional cursor, at most `limit` (1 to the offering maximum, else `Invalid`). `more` is true when another entry follows. Read-only; the price is the fixed discover price, an empty page included. Entries show the sender only as a hint.
- `recover(key, caller, input)`: verifies the recovery request, calculates the exact recovery price from the payload size and has the participant atomically record the provider-signed receipt and move the placement to recovered. Returns the original signed message and receipt plus the charged price (the output itself has no price field). An unknown, expired, non-pending or foreign placement, or a mismatched message or hash, is `NotAvailable`; a bad signature is `Invalid`. Nothing changes or is charged on failure.
