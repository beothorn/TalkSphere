# Ledger

Ledger will own issuer-separated accounting, trust, finite budgets and durable
settlement, transfers and claims for Services and authenticated Peer Protocol
adapters. It consumes public Identity/Cryptography capabilities and local platform
persistence, without accessing another module's private state.

Status: direct-call accounting, finite administration, all reservation modes,
participant recovery, canonical signed transfers, third-party confirmation and
secret claims are implemented with protected Linux persistence. Bounded read-only
`ledger-show` is available below. Final acceptance evidence is being audited;
Ledger network controls remain assigned to Peer Protocol composition.
See the [requirements map](REQUIREMENTS.md) and [33-step plan](../../../plans/6_Ledger_module/PLAN.md).
The map pins the [CR2SE](https://github.com/beothorn/CR2SE) revision and distinguishes
module evidence from future Services/network integration.

From the repository root, build with `cargo build --locked`, run the application
overview with `cargo run --locked -- --help`, and test with `cargo test --locked`.
Use the explicit inspection command below; mutations require authorized library contracts.

## Inspect from the repository root

`cargo run --locked -- ledger-show --ledger-store DIRECTORY --local-identity CR2SE_ID --account CR2SE_ID`
prints one existing protected account's exact balances, available capacity and
status counts. Run `cargo run --locked -- help ledger-show` for required options,
local limits and exit behavior. It requires exclusive store access and never
initializes missing state. Run `cargo test --locked --test ledger_cli` for executable
inspection scenarios; `cargo test --locked modules::ledger` exercises the library.
