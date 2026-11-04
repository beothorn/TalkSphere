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
