# Control composition

The driver owns trusted local requests, protocol registrations, socket admission,
shared budgets and clocks. Connection internals own framing/authentication and
stream semantics; application modules consume queued events through the public
control contract. This keeps socket policy out of protocol consumers.

Build with `cargo build --locked`, exercise with
`cargo test --locked modules::transport::driver`, and inspect CLI availability
with `cargo run --locked -- help serve`, from the repository root.
See [control contract](mod.md), [TCP adapters](../adapters/README.md) and the
[Transport guide](../README.md). Normal server composition still awaits real
Identity/Cryptography; passing test primitives proves orchestration only.
