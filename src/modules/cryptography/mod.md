# Cryptography module contract

The module exports `api` (local contracts), `config` (validated resource policy),
`error` (fixed failures), `ports` (entropy interface), `random` (platform adapter),
`secrets` (protected ownership) and `primitives` (implemented algorithms).
No backend or network listener is started by importing the module.

Identity, Transport and content codecs are callers. Platform entropy, authorized
key access and established algorithm libraries are dependencies. See the
[module guide](README.md) and [plan](../../../plans/2_Cryptography_module/PLAN.md)
for currently implemented operations and pending integration work.

`credential` validates public binding, `envelope` implements generic objects, and
`provider` composes the real local interfaces; the Transport adapter lives outside
protocol internals under application composition.

identity_keys provides protected generation/import, public credential derivation
and authorized platform sink transfer for Identity persistence.
