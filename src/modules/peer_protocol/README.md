# Peer Protocol

Peer Control will bind cr2se.peer version1 to authenticated Transport streams,
Board metadata, Services coordination and Ledger controls. Current status includes local authority, bounded record/control/value codecs,
canonical scalar/byte binding and provisional detached body reception/transmission;
provided-proposal authorization, immutable provider quotes, independent requester
validation, explicit spending consent and [durable two-sided acceptance](acceptance.md)
and [challenge-only remote check binding](checks/README.md) are also available.
A [bounded dependency worker pool](engine/worker.md) keeps synchronous module calls
outside the Transport reactor, retaining correlated late results without retry.
An opt-in [authenticated stream adapter](engine/bridge/mod.md) now connects
framing, worker ownership and bounded output through the real Transport API.
Complete handler orchestration and real node composition are still in progress.
The default server does not register a peer application protocol yet. Application composition supplies grants
and verified peer context; protocol internals do not own sockets or balances.

From the repository root:

```sh
cargo build --locked
cargo test --locked modules::peer_protocol
cargo test --locked --test peer_protocol_streams
cargo run --locked -- --help
```

See the [plan](../../../plans/8_Peer_Protocol_module/PLAN.md),
[requirements](REQUIREMENTS.md), [local contracts](contract.md) and
[upstream specification](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/peer-protocol/README.md).
No standalone service-node exercise is available until production composition is complete.

The Linux `peer_protocol_streams` test runs two real TCP endpoints with protected
Identity stores, actual Cryptography and Board metadata. It requires normal
completion before publishing observations and verifies malformed-stream isolation.
Its explicitly installed metadata-only test factory is not a full service node;
these are same-implementation instances in one process, not interoperability or
complete-services process evidence.
