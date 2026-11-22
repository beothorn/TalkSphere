# Transport Identity adapter

TransportIdentity.new owns a real Identity provider, its loaded handle, and separate
ReadPublic and Sign capabilities. It validates read access immediately and retains
the protected store and lock for its lifetime. The adapter exposes only Transport's
public credential projection, canonical ID derivation and signing of a Transport-created
AuthenticationMessage. Each signing call enforces the original Identity scope and
budgets. It exposes no private keys, backup/import operation or arbitrary peer signer.

Typed failures map to Transport's denied, invalid-input, verification-failed or
unavailable dependency categories. Resource limits map to invalid input; conflicts
and uncertain storage outcomes map to unavailable. Detailed Identity failures remain
available at local lifecycle boundaries. Transcript creation, role separation,
freshness, proof verification and session key publication remain Transport-owned.
This adapter does not start a socket server.

## Sharing with service modules

`TransportIdentity.with_decrypt(capability)` adds a `Decrypt` capability (the server credential loader grants 64 bytes, one
remote X25519 key per payload) before the identity is shared; it fails once a shared handle exists. `protected()` returns a
cloneable `ProtectedIdentity` over the same provider, handle and grants, so a process holds the credential store lock once.
`ProtectedIdentity` offers `public()`, `sign(bytes)` and `agree(remote public key)`; agreement is denied when no decrypt
capability was configured. Service modules build their own transcripts and key derivation on top; nothing exposes private keys.
