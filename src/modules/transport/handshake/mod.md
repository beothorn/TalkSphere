# Authentication construction exports

Exports Transcript, DirectionKeys, SessionKeys, derive_keys and nonce. These
are joined by Handshake, HandshakeError and Established for [state sequencing](state.md).
They form Transport's language-specific boundary to injected Identity/Crypto ports.
There are no mutable module globals or hidden runtime dependencies. The
[README](README.md) links the individual contracts and authoritative revision.
