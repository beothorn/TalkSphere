# Sealer

`Sealer::open_or_create(path)` reads the 32-byte data key at `path`, or creates a random one with owner-only permissions when
the file is absent. A key file that is not exactly 32 bytes or is readable by anyone but its owner is refused and never
replaced. Back the key up with the node's identity: without it no replica can be opened.

- `seal(plaintext, context)`: nonempty plaintext up to 65495 bytes; returns `version(1) || nonce(24) || ciphertext || tag(16)`,
  XChaCha20-Poly1305 with a fresh random nonce. `context` (the item identifier) is authenticated as associated data after
  the label `talksphere.replica.v1:`.
- `open(unit, context)`: authenticates version, nonce, tag and context before releasing any plaintext; anything else fails
  with `Authentication`, a unit too short to hold version, nonce and tag with `Size`.
- The overhead added to the plaintext is 41 bytes (`OVERHEAD`).
