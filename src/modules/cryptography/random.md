# Secure randomness

`SystemEntropy` implements the entropy contract using getrandom's platform CSPRNG
without fallback. `fill(output)` accepts up to `MAX_RANDOM_BYTES` (4096) bytes,
including empty output; larger requests fail with `ResourceLimit`, platform failures
with `Unavailable`. Output may be partially filled on error per the entropy port.

`random_bytes<N>(entropy)` requests exactly N bytes, rejects N above 4096 before
calling the dependency, and returns an owned zeroizing buffer. Failure discards
and erases any partially filled buffer. Composition supplies a trusted source;
deterministic substitutes remain test-only. No persistent keys are generated here.
The deliberate getrandom 0.3 API selection is pinned by Cargo.lock and supports the
current native build; unsupported platforms must fail rather than invent entropy.
