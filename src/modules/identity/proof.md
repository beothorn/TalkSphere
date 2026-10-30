# Exact protocol proof verification

Proof borrows the untrusted identity version, signing algorithm, claimed binary ID,
canonical public key, exact protocol message and signature. It contains no local
private handle. Version/algorithm one select Ed25519; ID and key are 32 bytes and
signature is 64 bytes.

verify(cryptography, proof, maximum message bytes) checks message budget and signature
size, validates the public ID/key relation, and then verifies the exact signature.
It returns the validated public identity only after all checks succeed. Malformed
values, unsupported selectors, ID mismatch, invalid proof, resource limits and
unavailable dependencies preserve their distinct error categories.

Public relation validation alone is not possession evidence. Successful proof
verification establishes validity only for these exact supplied bytes; the caller
must define domain separation, role, freshness and replay policy. The helper has no
clock, replay cache, trust state or connection-authenticated event. A repeated valid
proof remains cryptographically valid and must be rejected by the caller when its
protocol requires freshness. Transport owns its HELLO/AUTH state machine.
