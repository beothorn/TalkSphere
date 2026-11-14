# Incoming endpoint composition

The endpoint connects authenticated worker-owned exchanges to public Board,
Services and Ledger contracts. Application composition supplies those real local
owners, policies and independently installed families; the bridge supplies the
original authenticated endpoints, connection instance and deadline. The endpoint
owns wire orchestration, while module owners retain metadata, accepted work,
participant decisions, accounting and recovery. It owns no sockets or keys.

The provided-work/check and independent-control responder is implemented and
unit-tested over real module owners. Explicit outgoing metadata, provided work and independent control factories are
also implemented over a direct metadata cache. Paid/included outgoing check factories also preserve original Transport timing.
Wanted invitation/publisher factories preserve local input, consent and payer
roles independently of the stream opener. Explicit family-authorized cached retries
reuse the original requester operation and cannot start fresh work. [Real local node exercises](../../../../../docs/peer-protocol/NODES.md) cover complete
exchanges; default `serve` stays Transport-only.
Trusted composition can install the [protected claim adapter](../../../../composition/peer/claims.md)
with a real issuer Ledger and protected decryptor. A missing claim decryptor,
family or check rule is explicitly unsupported.

From the repository root:

```sh
cargo build --locked
cargo test --locked modules::peer_protocol::engine::endpoint
cargo test --locked --test peer_protocol_streams
cargo run --locked -- --help
```

The first test command exercises inline/detached work through real two-sided
reservation and commitment, paid/included evidence, exact signed Ledger transfers
and refusal paths under test-owned authenticated scopes. The integration-test command exercises TCP metadata isolation. The additional
[complete node exercises](../../../../../docs/peer-protocol/NODES.md) cover services,
wanted/check exchanges, three-party settlement and protected issuer process restart. See the
[endpoint contract](mod.md), [bridge contract](../bridge/mod.md),
[module guide](../../README.md) and the pinned
[upstream protocol](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/peer-protocol/README.md).

[Aggregate admission](resources.md) reserves transient value/body capacity before effects and bounds each authenticated peer across endpoint clones and connections. Transport framing and durable owner journals have independent quotas.
