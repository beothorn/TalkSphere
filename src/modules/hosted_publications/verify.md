# Hosted Publications verification

`verify(publication, expectedPublisher, bounds)` accepts a publication only when all of these hold, checked in this order: the identity
ID derived from the publisher's public key equals the publisher ID in the metadata and the expected publisher (the authenticated
requester for a host, the intended publisher for a reader); a resource has sequence zero; a public publication has no audience; a
restricted one has between one and `maximumAudience` strictly ascending, unique reader IDs; the content size is at least one, equals the
actual length and is within the smaller of `maximumContentBytes` and the 16384-byte profile limit; the SHA-256 of the content equals the
content hash; and the Ed25519 signature verifies over the exact signing bytes. Any failure is `Invalid` and names no field. `Bounds`
defaults to 64 readers and the profile budget.
