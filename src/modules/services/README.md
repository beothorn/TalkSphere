# Services

Services implements reusable typed validation, explicit handler registration and
durable local coordination over real Board and Ledger public contracts. Board
uses the schema validator and registered capabilities; trusted application
composition supplies authority and installed handlers; Peer Protocol will supply
authenticated stream events and wire encoding. No application protocol is yet
registered in production serve.

From the repository root:

```sh
cargo build --locked
cargo test --locked modules::services
cargo run --locked --example services
cargo test --locked --example services
cargo run --locked -- help services
```

The example explicitly registers a custom text handler and settles seven credits
from a finite hundred-credit grant (`price=7, remaining=93`). All example stores
and handler effects are volatile: each run starts fresh, and restart recovery is
not offered by the example. No standard-family runtime or network provider is
installed. `--help`/`-h` explains example usage and exit statuses.

For durable integration, composition must initialize absent Services, Ledger and
family stores explicitly, retain exact installed recovery code, and issue separate
scoped grants to each owner. Linux FileStore uses protected owner-only directories,
exclusive locking and synchronized atomic complete-image replacement. On Unknown,
stop affected owners, reopen all authoritative stores with fresh grants, then
resume original-key status and decisions. Never initialize missing history as a
repair, infer rollback from timeout or repeat execution to recover a lost reply.

The [implementation plan](../../../plans/7_Services_module/PLAN.md),
[requirements](REQUIREMENTS.md) and [coordinator contract](runtime/mod.md) describe
the lifecycle. Behavior and literal fixtures use
[CR2SE d40b6278f24430e814d7fa82094f57261bfcac9f](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f).
The coordinator owns local decision/result lifetime; families own their effects;
Ledger owns exact balances/payment. This separation permits recovery without
reading private dependency state or claiming atomic cross-peer settlement.
