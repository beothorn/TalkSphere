# Storage operation values

Operation identifies store, retrieve, renew or remove. Byte challenges use a
separate schema and never the ordinary invocation discriminator.

input takes a complete Services Value and finite Budget, checks duplicate fields,
aggregate depth/nodes/bytes, required fields for the selected operation and exact
logical kinds. Store requires positive size/duration, 32-byte hash and complete
bytes. Retrieve requires 32-byte ID, uint64 offset and positive length. Renew
requires ID, uint64 expected expiry and positive additional seconds. Remove
requires only ID. Known fields belonging to other operations are forbidden;
compatible unknown fields remain bounded inert data. No placeholder body is
accepted as complete bytes. Advertised limits, hashes and lease state are checked
by later owner/requester operations.

output takes the expected Operation, complete Value and Budget. It requires the
matching discriminator, exact 32-byte ID, positive uint64 price and each
operation's mandatory result fields and kinds. Removal requires removed=true.
Retrieval requires nonempty bytes and the whole-object hash. This shape check
does not compare original request, accepted price, retention or stored content;
requester validation must independently establish those relationships. Common
Services schema validation still applies to any additional known output fields.

challenge checks an ID and uint64 offset, rejecting invocation-specific known
fields, expected-byte material and provider outcomes. evidence requires ID,
offset and exactly one byte and rejects provider-computed outcome. Expected
bytes and local outcome never belong on a provider challenge/evidence path.

All functions borrow values, create no lease and authorize no spending. Invalid
indicates malformed or conflicting values; Limit indicates exhausted budgets.
