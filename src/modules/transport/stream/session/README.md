# Session operations

Authenticated sessions combine stream ownership, data/lifecycle state and
connection control. Probe timers and graceful drain logic are split into
`ping.rs` and `drain.rs`; the parent session owns their shared state.

Build with `cargo build --locked`, exercise with
`cargo test --locked modules::transport::stream::session`, and inspect runtime
status with `cargo run --locked -- help serve`, all from the repository root.
These operations use injected cryptography through the connection driver;
real credentials and normal CLI networking remain later integration work.
