# Protocol module exports

`transport` exports Transport framing, state, dependency ports and adapters.
`cryptography` exports the initial cryptographic contracts and validated policy;
it supplies no production cryptographic operations yet.

Modules communicate through explicit contracts. Application composition selects
adapters; exporting a module does not start a listener or enable a capability.

Identity and the remaining roadmap modules are future implementations; absent
modules must never be replaced with production authentication substitutes.
