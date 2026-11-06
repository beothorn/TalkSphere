# Ledger

Ledger will own issuer-separated accounting, trust, finite budgets and durable
settlement, transfers and claims for Services and authenticated Peer Protocol
adapters. It consumes public Identity/Cryptography capabilities and local platform
persistence, without accessing another module's private state.

Status: implementation in progress. Exact local accounting, finite administration,
all reservation modes, participant prepare/commit/abort/status and protected Linux
persistence are implemented and tested. Canonical transfer orders and protected
signing/verification are available; issuer transfer accounting and claim lifecycle
remain later plan steps. No Ledger network or CLI command is registered yet.
See the [requirements map](REQUIREMENTS.md) and [33-step plan](../../../plans/6_Ledger_module/PLAN.md).
The map pins the [CR2SE](https://github.com/beothorn/CR2SE) revision and distinguishes
module evidence from future Services/network integration.

From the repository root, build with `cargo build --locked`, run the application
overview with `cargo run --locked -- --help`, and test with `cargo test --locked`.
No Ledger CLI command is available yet.
