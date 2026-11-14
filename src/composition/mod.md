# Module composition

`cryptography` adapts the real cryptographic provider to Transport's dependency
port. Importing composition starts no services and supplies no Identity persistence.

`identity` supplies the real scoped Transport Identity adapter while retaining its
protected store. `server` defines validated application policy and lifecycle
contracts; runtime composition is being implemented through the integration plan.
Importing these contracts performs no filesystem or network effects.

[Peer composition](peer/mod.md) connects protected issuer claim dependencies to the opt-in endpoint.
