# Transport integrity tags

`tag(key, nonce, aad, maximum_bytes)` returns a 16-byte XChaCha20-Poly1305 tag
with empty plaintext. `verify_tag(key, nonce, aad, tag, maximum_bytes)` uses the
backend's constant-time tag verification and returns success or authentication
failure. Both require an Integrity capability and a 24-byte nonce; oversized AAD
returns `ResourceLimit` before work. AAD is the exact visible header and payload.

Transport owns directional key selection, prefix plus big-endian sequence nonce
construction, replay checks and counter exhaustion. This primitive does not hide
payloads, track sequences or reuse content encryption keys. Tests compare the
pinned first protected frame and reject wrong direction, nonce and altered bytes.
