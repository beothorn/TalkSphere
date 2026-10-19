# Platform entropy contract

`Entropy` is an injectable, transferable local platform dependency.
`fill(output)` receives a caller-owned bounded mutable byte buffer and either
fills it with fresh CSPRNG output or returns `Unavailable`. Empty buffers are
permitted. No production adapter is supplied by the scaffold.

On failure the output may be partially filled: the caller must discard/erase it
and cannot use it as key material. Production implementations must not silently
fall back to deterministic bytes, log output or retain a copy. Deterministic
adapters must be test-only. Composition chooses the adapter; peers cannot supply
it. Concrete secret cleanup and production randomness arrive in later steps.
