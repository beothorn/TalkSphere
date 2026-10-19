# Protocol module exports

`transport` exports Transport framing, state, dependency ports and adapters.
`cryptography` exports implemented primitives, generic envelopes, a real provider
and validated policy, with its acceptance audit tracked in the detailed plan.

Modules communicate through explicit contracts. Application composition selects
adapters; exporting a module does not start a listener or enable a capability.

Identity and the remaining roadmap modules are future implementations; absent
modules must never be replaced with production authentication substitutes.
