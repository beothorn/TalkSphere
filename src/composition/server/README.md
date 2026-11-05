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

[Credential startup](credentials.md) loads existing keys and explicitly selected
contacts before networking; `cargo test --locked --test server_credentials` checks
missing/corrupt state, scoped public projection and exclusive lock lifetime.

[Real dependency composition](transport.md) shares TCP readiness with the monotonic
clock and supplies production CSPRNG Cryptography plus scoped Identity to Driver.
Construction binds no sockets and retains private authority until the driver drops.

[Bounded event polling](pump.md) retains readiness notifications and local shutdown
requests, yields after 64 events and waits on a condition variable instead of spinning.
Run `cargo test --locked composition::server::pump` for deterministic scheduling checks.

Transport.listen returns the actual bound address and public ID after binding and
retains listener ownership. Lifecycle transitions prevent duplicate startup; tests
verify port-zero readiness and that dropping the composition releases the port.

[Contact startup](contacts.md) initiates each configured contact once with its exact
expected ID and preserves per-attempt failures without taking down the listener.
Admission is not authentication; no implicit reconnect or Discovery is performed.
