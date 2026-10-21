# Platform entropy contract

`Entropy` is an injectable, transferable local platform dependency.
`fill(output)` receives a caller-owned bounded mutable byte buffer and either
fills it with fresh CSPRNG output or returns `Unavailable`. Empty buffers are
permitted. The production [SystemEntropy adapter](random.md) uses platform CSPRNG output.

On failure the output may be partially filled: the caller must discard/erase it
and cannot use it as key material. Production implementations must not silently
fall back to deterministic bytes, log output or retain a copy. Deterministic
adapters must be test-only. Composition chooses the adapter; peers cannot supply
it. [Scoped secrets](secrets.md) erase owned temporary material on every exit.
