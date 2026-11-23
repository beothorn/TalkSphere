# Hosted Publications operations

Logical inputs and outputs of the five operations and the check; the authenticated requester is supplied by Services and is never a field.

- `StoreRequest`: the complete signed publication and `retentionSeconds`. `StoreOutput`: `placementId`, `publicationId`, `acceptedAt`, `expiresAt`
  (acceptance plus retention); the price is reported beside it.

- `ListRequest`: `publisherId`, `kind` (page or feed-post), optional exclusive cursor `(sequence, publicationId)` (both parts or neither) and `limit`.
  `ListEntry`: placement ID, publication ID, kind, sequence, claimed time, visibility, content size and hash, acceptance and expiry. `ListOutput`: the
  publisher, the entries and `more`; the next cursor is the last entry's sequence and publication ID.

- `GetRequest`: `publisherId`, `publicationId` and `maximumBytes` (`1 ..= maximumContentBytes`). `GetOutput`: the complete publication exactly as stored.

Later steps add renew, remove and check values here.
