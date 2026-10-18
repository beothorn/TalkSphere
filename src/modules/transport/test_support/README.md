# Transport test helpers

Step 2 provides deterministic substitutes for the step 1 dependency ports.
This entire module is behind `#[cfg(test)]` and is visible only within the
crate's unit-test build. Ordinary builds, including release builds, cannot use
these helpers; there is no runtime option or Cargo feature to enable simulated
authentication.

## Helpers and behavior

| Helper | Test control and observations |
|---|---|
| `socket::ScriptedSocket` | Queue read chunks, explicit EOF and I/O errors; allow a chosen number of bytes per write; inspect accepted bytes and closure. |
| `network::ScriptedNetwork` | Expect exact connect/listen endpoints and supply in-memory connection attempts, listeners or immediate errors. |
| `network::ScriptedConnecting` / `ScriptedListener` | Complete pending connections or enqueue accepted sockets/errors, inspect closure and test ownership transfer without TCP. |
| `clock::ManualClock` | Share a control clone, advance a supplied monotonic origin explicitly, and wake the latest registered deadline. |
| `identity::ScriptedIdentity` | Expect public-identity, key-identification and signing calls with exact input bytes and explicit success/failure results. |
| `crypto::ScriptedCryptography` | Expect every primitive call, its public inputs and secret labels, with explicit nonce/key/hash/signature/agreement/tag results or failures. |
| `crypto::SecretLog` | Issue non-cloneable test secret labels and observe their release, including consumed inputs on failed calls. |
| `consumer::RecordingConsumer` | Retain events and bytes in order; return an undelivered event when its event-count capacity is full; drain it to restore capacity. |
| `wake::WakeCounter` | Count wakeups without an executor or sleeps. |
| `vectors` | Load the pinned upstream byte fixtures without computing protocol encodings or cryptography. |

Empty socket read/write queues mean `Pending`, not EOF or successful output.
Use `ReadStep::Eof` to end input at an exact byte offset, one-byte chunks to
fragment it, or one chunk containing several fixed frames to coalesce them.
Writing is allowed only by explicit `WriteStep::Accept` entries; zero is useful
for exercising zero-progress failures. Adding readiness wakes the latest
waiter for that direction. Closing/dropping a socket wakes pending I/O and
prevents further work; inspection handles can outlive the socket.

Connection attempts have one completion, while listeners accept a queue of
results. Closing or dropping either releases queued sockets but never closes
one already transferred to its caller. These are simulated adapters, not the
Mio TCP implementation under adapters/.

The manual clock reads its supplied origin only once; time changes exclusively
through `advance`. Choose one origin per test, derive deadlines from it, and
advance by exact durations. Overflow is an error without changing the clock.
Replacing the deadline/waker discards the old registration.

## Script assertions

Construct `script::Script` from ordered `(expected_call, reply)` pairs, keep an
inspection clone and pass another clone to its adapter. Check `calls()` when a
test needs the complete input trace and call `assert_finished()` after the
expected operations. Wrong input/order, extra calls, missing calls and wrong
reply variants fail the test rather than returning a default successful result.
Socket scripts have their own `SocketControl::assert_finished()` check.

Adapter errors are explicit `DependencyError` or I/O error values. Scripts can
reject signatures, key agreement and tags; they do not calculate whether those
values are valid. Secret labels contain no private material, and observing
their Drop does not prove real key erasure. Real cryptographic vectors remain
pending until the real adapters exist.

The recording consumer observes events provided by the driver or a test;
it performs no authentication, dispatch or stream-state validation. Its
event-count limit is a way to simulate a stalled consumer, not production byte
budget enforcement. Test-only handle constructors in `handles::test_fixtures`
support these observations without exposing capability constructors to normal
builds.

## Fixtures

See [fixtures/README.md](fixtures/README.md) for the upstream revision and exact
snapshot. The loader preserves byte values and rejects malformed hex or wrong
array sizes. Tests check fixture structure and exercise arbitrary truncation
and fragmentation of its fixed frame bytes; they do not claim Transport
acceptance coverage or cryptographic interoperability.

## Build, run and test

From the repository root:

```sh
cargo build --locked
cargo test --locked modules::transport::test_support
cargo test --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo run --locked -- --help
```

The helper tests require no real sockets, sleeps, cryptographic libraries or
external services. `cargo run --locked -- serve` still reports networking as
unimplemented and exits with status 1. See the [Transport README](../README.md)
and [module plan](../../../../plans/1_Transport_module/PLAN.md) for next steps.
