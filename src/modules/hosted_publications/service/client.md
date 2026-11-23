# Service client

The requester contract installed for both roles. Services validates quotes, inputs, results and paid checks through it.

- `Placements`: the port through which the requester states what it independently knows about a placement (its content size). `Denied` for an unknown placement. A provider can never choose the size a renewal
  or check is based on.
- `Client::new(definition, catalogue, placements, values)`: accepts only a definition the hosted publication catalogue recognizes as the pinned contract; value bounds must be nonzero and at most depth 31.
- Pricing (`pricing_with_peer`): exact fixed prices from the offering. A store is priced from the body-free projection (size within the offering, retention within its limits, empty factors); a list from its fixed
  price (kind page or feed post, limit within the offering); a get needs exactly the factor `contentSize`, which must be positive, no larger than the `maximumBytes` the requester chose and no larger than the
  offering, and is priced from it; a renewal needs exactly the factor `contentSize` equal to the size the requester knows for that placement; a removal needs exactly `releaseCredits`, not below the publisher's
  `minimumReleaseCredits`. Anything else is `Invalid`. Unscoped pricing and factor validation are `Unsupported`.
- Projection and input: the assessment metadata is the body-free store projection or the complete request; `validate_input` requires the complete input to match the accepted projection.
- `validate_result`: the response must belong to its request. Store: same publication, expiry equals acceptance plus retention. List: the publisher, at most `limit` entries, all of the requested kind, strictly
  descending by `(sequence, publicationId)` and strictly after the cursor. Get: the publication's ID and publisher are those asked for, its size is within `maximumBytes` and equals the quoted `contentSize`, and the
  publisher's signature, hash and rules verify. Renewal arithmetic and removal (at least the minimum credits) as for Messaging.
- `cached_retry`: only removals, by their removal ID; reads and other operations have none.
- Check rules (`Rules`): every challenge is separately paid; the challenged offset must lie inside the content the requester knows; evidence must answer exactly the challenge; `outcome` is pass or fail only for timely,
  matching evidence against a private expectation `{placementId, offset, expectedByte}`, otherwise inconclusive.
