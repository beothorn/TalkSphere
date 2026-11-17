# Validate a returned store lease

validate takes a trusted original Services Invocation, complete original input,
complete authenticated output, independent Value Budget and requester Policy.
Policy specifies independently observed UTC Unix seconds and a permitted apparent
clock difference; neither comes from the provider's output.

The verifier checks matching installed definition/accepted offering identity,
requester/provider/publisher/issuer associations, exact shapes and integer widths,
original byte length and SHA-256, returned size/hash, exact accepted work price,
checked acceptance-plus-retention expiration and local clock policy. It preserves
the original accepted terms as an opaque immutable binding without decoding
Services' private journal representation. Lease IDs are exactly 32 bytes; the
requester cannot infer randomness from one returned identifier.

The returned Lease is owned validated metadata. Normal stream completion and
local coordinator commitment remain caller responsibilities; this function alone
neither proves provider durability nor authorizes catalog publication. It changes
no balances, trust or resources. Denied indicates inconsistent original authority,
Invalid inconsistent content/terms, Unsupported another contract and Limit bounds
or expiration overflow. Schema failures retain their existing typed errors.

The acceptance-plus-retention equation is verified without selecting a resolution
for upstream acceptance-boundary issue #11. No precommit timestamp is asserted to
be a valid durable activation time by this verifier.

Active records successful creation metadata, not a fresh observation of current
provider availability after arbitrary delivery delay. Catalog callers retain
original times and apply their own renewal/retrieval policy.
