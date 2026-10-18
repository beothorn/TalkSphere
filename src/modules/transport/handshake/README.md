# Authentication constructions

Handshake state uses role-ordered transcripts and directional key
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

Step 8 adds [sequencing and identity gates](state.md), including same-identity
peers, contact pins, duplicates and independent tag transitions. Tests combine
AUTH and a tagged frame in one socket chunk. Derivation follows the normative
post-AUTH ordering; [bug 0002](../../../../CR2SE%20bugs/0002-key-derivation-before-auth-verification.md)
records the differing pseudocode order. The integrity layer performs tag verification through injected Crypto.

`serve` remains unavailable. Constructors never authenticate by themselves;
only fully checked state can release connection facts. Real cryptography and
normal server composition remain required for a running node.
