# Hosted Publications identifiers

`PublicationId` and `ContentHash` are exactly 32 bytes. `PublicationId` is chosen by the publisher with a secure random generator and
is shared by every placement of the publication; `ContentHash` is SHA-256 of the content. `from_slice` accepts exactly 32 bytes and
fails with `Invalid` otherwise. Both order by their bytes (unsigned lexicographic), which the list cursor relies on, and never print
their value. `PlacementId`, `RemovalId` and `Signature` (64 bytes) are the [Messaging](../messaging/ids.md) types with the same meaning.
