# Cryptography module contract

The module exports `api` (local requests and capabilities), `config` (validated
resource policy), `error` (fixed failure categories) and `ports` (platform entropy).
This scaffold provides no production primitive or envelope implementation and
claims no cryptographic conformance. No backend is selected automatically.

Identity, Transport and future content codecs are callers; only platform entropy,
protected key access and established algorithm implementations are dependencies.
See [the module guide](README.md) for ownership and implementation status.
