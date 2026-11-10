# Peer Protocol

Peer Control will bind cr2se.peer version1 to authenticated Transport streams,
Board metadata, Services coordination and Ledger controls. Current status is
requirements reconciliation and local authority/context contracts; no peer
application protocol is registered yet. Application composition supplies grants
and verified peer context; protocol internals do not own sockets or balances.

From the repository root:

```sh
cargo build --locked
cargo test --locked modules::peer_protocol
cargo run --locked -- --help
```

See the [plan](../../../plans/8_Peer_Protocol_module/PLAN.md),
[requirements](REQUIREMENTS.md), [local contracts](contract.md) and
[upstream specification](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/peer-protocol/README.md).
No runnable network exercise is available until the composition steps are complete.
