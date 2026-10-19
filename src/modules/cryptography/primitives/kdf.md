# HKDF-SHA-256

`extract(salt, shared, maximum_bytes)` consumes a protected 32-byte X25519 result
and returns a protected PRK, preserving exact salt bytes. Oversized salt fails
before processing and still releases consumed authority.
`expand_key<P>(prk, info, maximum_bytes)` returns a protected 32-byte key with the
construction-selected purpose. `expand_nonce_prefix(prk, info, maximum_bytes)`
returns a public 16-byte Transport nonce prefix. Info is bounded and unchanged;
callers own exact labels and separators. Backend failures map to structured errors.

All expansion checks RFC5869's 255*32-byte output bound before backend work.
The current public constructions expose only the required 32/16-byte outputs.
Tests additionally exercise empty, maximum and oversized expansion plus RFC5869
case 1 and all pinned envelope intermediates.

HKDF 0.13 uses SHA-256/HMAC with zeroization features enabled deliberately; SHA-256
was upgraded to 0.11 for keyed-state cleanup. Owned PRKs and key buffers erase on
drop, and extraction's returned temporary is explicitly erased. As documented for
secret ownership, compiler/backend temporary copies are not a physical-erasure
proof. Cargo.lock pins the versions; hmac is direct to enable its zeroize feature.
