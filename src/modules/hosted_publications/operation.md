# Hosted Publications operations

Logical inputs and outputs of the five operations and the check; the authenticated requester is supplied by Services and is never a field.

- `StoreRequest`: the complete signed publication and `retentionSeconds`. `StoreOutput`: `placementId`, `publicationId`, `acceptedAt`, `expiresAt`
  (acceptance plus retention); the price is reported beside it.

Later steps add list, get, renew, remove and check values here.
