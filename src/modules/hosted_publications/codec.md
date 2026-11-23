# Hosted Publications codec

Maps typed requests and responses to the Services logical value model. Requests: `store` (publicationId, publisherId, kind, sequence, publishedAt,
visibility, audience as an array of 32-byte IDs, contentSize, contentHash, publisherPublicKey, publisherSignature, retentionSeconds, content), `list`
(publisherId, kind, limit as uint32 and either both `beforeSequence` and `beforePublicationId` or neither), `get` (publisherId, publicationId,
maximumBytes), `renew` (placementId, expectedExpiresAt, additionalSeconds) and `remove` (placementId, removalId, expectedExpiresAt,
minimumReleaseCredits). Responses carry the specification's fields and `price`; list entries and the get publication mirror the stored metadata.

Rules: an object with unique names; every required field with its exact type and length; content length equal to `contentSize` and at least one; a
limit or `maximumBytes` of zero is `Invalid`; a single cursor field is `Invalid`; a field belonging only to another operation is `Invalid` while an
unknown field is ignored (compatible extension); an unknown operation, kind or visibility is `Unsupported`. Check challenges carry `placementId` and
`offset`; check evidence carries the same plus exactly one `value` byte. Wire encoding belongs to Peer Protocol.
