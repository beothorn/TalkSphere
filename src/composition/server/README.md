# Authenticated server composition

Status: validated policy and lifecycle contracts are implemented; runtime execution
follows the [integration plan](../../../plans/4_Transport_integration/PLAN.md).
CLI and trusted application callers supply local configuration. Composition will
own protected Identity, real Cryptography, platform adapters and the Transport
Driver; it does not implement application protocols or service transactions.

From the root, run `cargo build --locked`, `cargo test --locked composition::server`
and `cargo run --locked -- --help`. The serve command remains explicitly unavailable
until runtime startup and shutdown are implemented. See [policy](config.md),
[lifecycle](mod.md) and the [upstream evidence inventory](../../../docs/transport-integration/ACCEPTANCE.md).

Runtime fixtures are test-only: `cargo test --locked --test runtime_helpers` checks
isolated child output/cleanup, and `cargo test --locked composition::server` checks
wake/shutdown probes. Existing Transport ManualClock and scripted socket boundaries
remain available for deterministic timing and I/O; authentication tests use real keys.
