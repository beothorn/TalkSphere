# Hosted Publications metadata

`Kind`: `page` (code 1, content is a page manifest), `feed-post` (2, a feed post) or `resource` (3, one page resource). `name()` gives the
wire name, `from_name` refuses anything else with `Unsupported`; pages and feed posts are listed, resources are addressed only by ID.
`Visibility`: `public` (code 0) or `restricted` (1).

`Metadata`: `publicationId`, `publisherId`, `kind`, `sequence` (zero for resources), `publishedAt` (the publisher's claimed Unix
seconds, not a trusted clock), `visibility`, `audience` (strictly ascending reader IDs, empty when public), `contentSize` (at least one)
and `contentHash` (SHA-256 of the content). `Publication` adds the publisher's 32-byte Ed25519 public key, the 64-byte signature and the
exact content bytes (ciphertext when restricted). Validation of these rules is `verify` (plan step 4).
