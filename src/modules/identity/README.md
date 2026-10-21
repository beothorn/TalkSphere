# Identity

Status: public contracts and resource policy are implemented; identity creation,
protected storage and operations remain pending the [plan](../../../plans/3_Identity_module/PLAN.md).
No scaffold returns fake credentials. This is a direct in-process boundary.

Identity owns persistent credentials, scoped authority and configured contacts.
It calls Cryptography for algorithms and a protected platform store for durability;
Transport owns sessions and fresh proof transcripts. Ledger owns trust and balances,
and the application owns its recovery catalog. No Discovery or paid Storage runtime
is needed. Composition lives outside protocol internals.

Reviewed baseline: [CR2SE d40b6278](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/identity),
including the [inputs](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/identity/inputs/README.md),
[outputs](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/identity/outputs/README.md),
[pseudocode](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/identity/pseudocode/README.md)
and [acceptance cases](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/identity/acceptanceTests/README.md).
See [API](api.md), [limits](config.md), [errors](error.md) and [ports](ports.md).

From the repository root:

```sh
cargo build --locked
cargo test --locked modules::identity
cargo test --locked --doc
cargo doc --locked --no-deps
cargo run --locked -- --help
cargo run --locked -- help serve
```

There is no Identity CLI command yet. `cargo run --locked -- serve` returns status 1
until Identity and normal server composition exist. No listener is opened.
Production coverage follows [the shared procedure](../../../docs/coverage.md).

[Test helpers](test_support/README.md) pin the reviewed upstream fixtures and
provide ordered dependency scripts and restart/failure substitutes.

Cryptography now exposes canonical public signing-key validation for Identity;
ID derivation and the Identity-facing provider remain subsequent steps.
