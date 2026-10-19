# Real in-process provider

`Provider.system(maximum_bytes)` selects platform CSPRNG entropy.
`Provider.new(entropy, maximum_bytes)` injects a trusted local entropy dependency;
deterministic adapters are test-only. Both validate a ceiling of at least 148 bytes
without consuming randomness. `maximum_bytes()` exposes the immutable ceiling.
`hello_nonce()` requests a fresh public 32-byte nonce or returns entropy failure.

The provider implements Hashing, Signatures, KeyAgreement and IdentityEncryption
from [the API contract](api.md) with the real primitive/envelope operations. It
preserves scoped key purposes, exact bytes, strict signature policy and structured
failures. Envelope request budgets are intersected with the provider ceiling;
callers cannot enlarge the provider budget. It owns no persistent credentials,
trust state, network endpoint or remote signing service. Private capabilities are
authority supplied by authorized local composition.
