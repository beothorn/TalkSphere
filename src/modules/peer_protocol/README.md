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
An [incoming endpoint factory](engine/endpoint/README.md) now composes provided
work/checks, metadata and independent Ledger controls over actual owners.
Explicit outgoing metadata, provided work/checks, wanted invitations, cached
retries and independent controls use one-shot request factories and direct-publisher
cache checks. [Real local node exercises](../../../docs/peer-protocol/NODES.md)
cover complete work, evidence, wanted roles and independent third-party settlement,
including three processes and protected issuer-record restart.
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
Run the explicit custom-handler node exercises with:

```sh
cargo test --locked --lib modules::peer_protocol::engine::endpoint::tests::network -- --nocapture
```

These fixtures use real public module owners and protected credentials; their
custom service resources remain explicitly volatile. The default server still
requires opt-in application composition to register this protocol.

The Linux `peer_protocol_streams` test runs two real TCP endpoints with protected
Identity stores, actual Cryptography and Board metadata. It requires normal
completion before publishing observations and verifies malformed-stream isolation.
Its explicitly installed metadata-only test factory is not a full service node;
these are same-implementation instances in one process, not interoperability or
complete-services process evidence.
