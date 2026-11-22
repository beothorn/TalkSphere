# Native Messaging exercise

From the repository root on Linux with loopback networking:

```sh
cargo build --locked
cargo run --locked -- help messaging
cargo run --locked --example messaging -- --help
cargo run --locked --example messaging -- help run
cargo run --locked --example messaging -- run
cargo test --locked --example messaging
```

Run creates four nodes in one process, each with its own protected identity file, its own listener and its own native Board,
Services and Ledger journals: a sender, a recipient and two providers. The providers also own native Messaging placements and a
participant journal and recover them before the offering is published. The nodes authenticate pinned public identities over
loopback TCP. The same protected identity authenticates each connection, signs messages and recovery requests, and decrypts the
recipient's payloads; private keys never leave Identity (see [Identity agreement](../../src/modules/messaging/identity.md)).

What the exercise checks, in order:

1. Each requester connects to both providers and fetches the provider's Board and definition directly from it.
2. The sender encrypts one message to the recipient's published encryption key, signs it and stores it at each provider at the
   exact quoted price (a replica; one placement is one copy at one provider). Its balance at each provider falls by that price.
3. The recipient discovers the message at both providers, recovers it from the first with a signed request, verifies the sender's
   signature, the payload hash and the provider's receipt, decrypts it and receives the exact plaintext once. Recovering the
   replica at the second provider is reported as a duplicate, so the message is delivered once.
4. The sender reads the status (with the provider-signed receipt) and renews the first placement.
5. A second, unread message is stored at the second provider. The sender judges one paid byte check against a private
   expectation, then removes the message. The removal releases credits to the sender exactly once, net of the removal price.

The exercise uses finite demonstration balances and removes its temporary directories after shutdown. It reuses the node
composition of the peer and storage examples. The Storage exercise covers a provider process and reopen; Messaging's crash and
reopen behavior is covered by the module tests (`messaging::service::crash`), not by this loopback profile. Offering bounds are
widened in this example (1024-byte payloads, up to 600 seconds) because the pinned fixture's 100-byte bound is smaller than an
encrypted payload. Default `serve` and `node-init` do not install or publish Messaging; the client journey plan adds that.

Help exits 0 without opening stores or sockets. Invalid syntax exits 2 and a runtime failure exits 1. Use
`RUST_LOG=talksphere::modules::messaging=trace` for Messaging traces on stderr. The completion line goes to stdout.
