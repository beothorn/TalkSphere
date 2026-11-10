# Peer Protocol

Peer Control will bind cr2se.peer version1 to authenticated Transport streams,
Board metadata, Services coordination and Ledger controls. Current status includes local authority, bounded record/control/value codecs,
canonical scalar/byte binding and provisional detached body reception/transmission;
provided-proposal authorization, immutable provider quotes, independent requester
validation, explicit spending consent and [durable two-sided acceptance](acceptance.md)
and [challenge-only remote check binding](checks/README.md) are also available.
Exchange orchestration and real module/network composition are still in progress.
No peer application protocol is registered yet. Application composition supplies grants
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
