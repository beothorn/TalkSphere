# Service client

The requester contract installed for both roles. Services validates quotes, inputs, results and paid checks through it.

- `Placements`: the port through which the requester states what it independently knows about a placement (its payload size). `Denied` for an unknown placement. A provider can never choose the size it is paid for.
- `Client::new(definition, catalogue, placements, values)`: accepts only a definition the Messaging catalogue recognizes as the pinned contract; value bounds must be nonzero and at most depth 31.
- Pricing (`pricing_with_peer`): exact fixed prices from the offering. A store is priced from the projection (size and retention within the offering limits, empty factors); discover and status from their fixed prices; a recovery or renewal needs exactly the factor `payloadSize` and it must equal the size the requester knows; a removal needs exactly `releaseCredits` and it must not be below the sender's `minimumReleaseCredits`. Anything else is `Invalid`. Unscoped pricing and factor validation are `Unsupported`.
- Projection and input: the assessment metadata is the body-free store projection or the complete request; `validate_input` requires the complete input to match the accepted projection.
- `validate_result`: the response must belong to its request (matching message, placement, replay IDs; store expiry equals acceptance plus retention; renewal arithmetic; removal at least the minimum credits). Signatures and decryption are checked by the receiving client.
- `cached_retry`: recoveries and removals are identified by their replay IDs; other operations have none.
- Check rules (`Rules`): every challenge is separately paid; the challenged offset must lie inside the payload the requester knows; evidence must answer exactly the challenge; `outcome` is pass or fail only for timely, matching evidence against a private expectation `{placementId, offset, expectedByte}`, otherwise inconclusive.
