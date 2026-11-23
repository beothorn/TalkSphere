# Hosted Publications errors

`Invalid` (malformed input), `Unsupported` (unknown version, pricing model or kind), `NotAvailable` (unauthorized or unknown, deliberately
indistinguishable), `Limit`, `Conflict` (stale expectation, duplicate publication, conflicting sequence or replay), `Insufficient`,
`Storage` (failed before commitment), `Unknown` (outcome may be committed; recover, never retry blindly) and `Unavailable` (clock,
entropy or signer missing). They map one to one to the Services errors and to Messaging's; `NotAvailable` is Services `Denied`. Errors
carry no identifiers, content or peer text.
