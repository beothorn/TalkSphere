# Authentication constructions

The future handshake driver uses role-ordered transcripts and directional key
derivation here. Identity supplies credentials/signatures; Crypto supplies
primitives and opaque secret capabilities. This module owns domain separators,
role bytes, length prefixes, HKDF labels and local send/receive assignment.

Reference: [CR2SE revision 8e848eb, section 5](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/README.md#5-mandatory-authenticated-handshake).
Step 7 checks exact primitive inputs and script-returned assignments, not real
signature/key/tag values. All-zero agreement rejection belongs to Crypto's
opaque `agree` contract, which Transport propagates. Secret capabilities are
consumed/released on failure; actual erasure still needs real adapters.

See [exports](mod.md), [transcript](transcript.md) and [keys](keys.md).
From the repository root:

```sh
cargo build --locked
cargo test --locked modules::transport::handshake
cargo test --locked
cargo run --locked -- --help
```

`serve` remains unavailable. No connection becomes authenticated by calling a
constructor alone; sequencing, peer checks and dispatch gates are subsequent work.
