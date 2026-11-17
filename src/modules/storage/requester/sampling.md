# Secure optional check sampling

`offset` takes a trusted cryptographically secure entropy dependency, positive
stored size in bytes and positive maximum draw count. It returns a uniformly
distributed zero-based offset below that size. The requester sends the chosen
offset only when the challenge begins; it never preannounces samples or asks
the provider to choose them.

Each draw reads eight fresh protected bytes and interprets them as an exact uint64.
Let threshold be 2^64 modulo size; reject draws smaller than threshold, then
return draw modulo size. The accepted bucket has an exact multiple of size
members, avoiding modulo bias even near uint64::MAX. Size arithmetic uses no
floating point and allocates no object-sized buffer. At most maximum draws are
attempted; a rejected sample consumes a draw.

Zero size/effort returns Invalid, entropy failure Unavailable, and exhausted
rejection effort Limit. Failed/partial randomness never produces a fallback
sample, and protected temporary buffers are erased by the existing Cryptography
randomness contract. No offset, expected byte, key or object payload is logged.

Production composition supplies platform CSPRNG entropy; deterministic substitutes
are test-only. Run `cargo test --locked modules::storage::requester::sampling`
from the repository root.
