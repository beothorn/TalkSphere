# Storage

Contract foundations for [CR2SE Storage version 1](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/README.md).
Implementation is in progress under the [38-step plan](../../../plans/9_Storage_module/PLAN.md).
There is currently no registered paid Storage provider or runnable Storage command.

Services will call requester/provider contracts and coordinate Ledger decisions.
Storage owns immutable byte leases, durable effects and results; Board owns
publication and Peer Protocol handles authenticated exchanges. Storage consumes
injected clock, random/hash and local durability ports, never another family's
runtime or another owner's private database. Application encryption and recovery
catalogs remain requester responsibilities. One lease is one copy.

From the repository root:

```sh
cargo build --locked
cargo test --locked modules::storage
cargo run --locked -- --help
```

See [requirements and port audit](REQUIREMENTS.md) and the plain-language
[contract](contract.md). No-argument application invocation shows help; use
`cargo run --locked -- help <topic>` for detailed existing command help.

Test-only deterministic ports model wall-clock rollback, collisions, entropy and
hashing failure, bounded partial reads and concurrent durable writes. Restart
reconstructs an adapter from retained bytes; lost replies preserve commitment.
These helpers supply no production authentication or durability.

Pinned complete definitions, terms, hash and literal prices are recorded in
[fixture provenance](fixtures/README.md); these pass independent schema/hash
checks and do not establish provider conformance.

[Exact pricing](pricing.md) implements single-round store and usage calculations
with a documented overflow argument; it does not register a Storage provider.
