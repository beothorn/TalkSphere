# Frame integrity

The connection driver supplies established directional keys to Integrity, which
owns two implicit sequences and delegates tag operations to Crypto. It depends
on canonical frame headers, bounded WireFrame payloads and Transport's nonce
construction. It neither encrypts content nor chooses stream/error scope.

Reference: [CR2SE revision 8e848eb, section 5.4](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/README.md#54-post-handshake-integrity-tag).
Step 9 checks exact AAD, direction/sequence requests, release on rejection and
exhaustion with strict scripts. Actual cryptographic mutation resistance remains
pending real Crypto integration; returned fixture tags do not establish it.

seal reserves one outbound sequence, and sent commits it only after complete
frame output. A sealed frame must not be discarded/reordered and then retried
on the same session. One owner must serialize seal/write/sent; on any write
failure it closes the socket and Integrity. Receive verification advances only
after a complete successful tag check. Missing/failed tags close without a wire
error response and produce no VerifiedFrame. The driver must obey that outcome.

See [public contracts](mod.md). From the repository root:

```sh
cargo build --locked
cargo test --locked modules::transport::integrity
cargo test --locked
cargo run --locked -- --help
```

`serve` is still unavailable. Connection composition and acceptance verification
are composed by the connection driver; source operations log context but never bytes, tags or keys.
