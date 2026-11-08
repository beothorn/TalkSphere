# Services

The reusable schema-format validator is implemented for Board. Services authority,
versioned call contracts and logical value validation are being added under the
implementation plan. Handler execution and durable coordination remain unfinished. Board is the current caller; future Services
and Peer Protocol will reuse these descriptions. There is no handler execution,
spending or network dependency. The format is specified by
[CR2SE Services](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/services/README.md).

From the repository root: cargo build --locked; cargo test --locked
modules::services; cargo run --locked -- --help. No Services command exists.

The [Services implementation plan](../../../plans/7_Services_module/PLAN.md)
is in progress; its checklist records verified completion.

[Requirements and acceptance obligations](REQUIREMENTS.md) map upstream cases to
planned steps; runtime conformance has not been established. Logical values retain
exact integer widths and float bits; shape validation is independent of family
semantic validation. Run focused checks with `cargo test --locked modules::services`.
