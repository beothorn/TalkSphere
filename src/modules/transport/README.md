# Transport

Transport's planned implementation is complete with injected dependencies:
wire codecs, bounded fragmented I/O, handshake and directional integrity
orchestration, multiplexed streams, cancellation, ping, graceful drain, deadlines,
resource limits, fair scheduling, TCP adapters and the public control driver.

Real Identity and Cryptography modules are still pending. The normal `serve`
command returns `TransportError::RuntimeUnavailable`, opens no sockets and exits
1. Test substitutes are unavailable in production builds. Local test success
does not establish cryptographic conformance or an authenticated deployment.

## Callers and boundaries

Trusted composition constructs `driver::Driver` from validated `TransportConfig`
and `Dependencies`, then uses `api::TransportControl::submit` and `poll_event`.
It owns protocol registration and routes queued events to consumers. Identity
supplies an existing credential and purpose-scoped signing authority;
Cryptography supplies primitives and opaque secret lifetimes. TCP/clock adapters
provide nonblocking OS I/O and monotonic wakeups. Transport owns no application
retry, credit, ledger rollback or service-specific message semantics.

Connection and stream capabilities are process-local and scoped to exact
instances/owners. Registration grants authority; peer IDs and wire stream IDs
do not. New connections cannot revive stale handles. Authentication is emitted
only after peer verification and successful local AUTH output.

Frame integrity authenticates header and payload bytes but does not encrypt
application bytes. Incoming DATA is dispatched only after verification. DATA
frame boundaries have no application meaning: concatenate bytes per direction.
Local queue admission is not proof of remote delivery or application success.

## Organization

| Domain | Responsibility |
| --- | --- |
| [API](api.md), [handles](handles.md), [configuration](config.md) | Trusted local contracts, authority and finite policy |
| [Ports](ports/mod.md) | Explicit Identity/Crypto/socket/clock dependencies |
| [Codecs](codec/README.md) | Header, control, handshake and OPEN byte formats |
| [Framing](framing/README.md) | Complete frames across partial/coalesced socket I/O |
| [Handshake](handshake/README.md) | Role-bound transcript, pinning and key requests |
| [Integrity](integrity/README.md) | Directional counters, tags and replay rejection |
| [Streams](stream/README.md) | Registration, ID history, DATA/lifecycle and scheduling |
| [Failures](failure.md), [resources](resources.md) | Error scope and shared admission/byte reservations |
| [Connection](connection/README.md) | One socket's composed state, deadlines and cleanup |
| [Driver](driver/README.md) | Public control requests, listeners and fair connection polling |
| [Adapters](adapters/README.md) | Mio TCP readiness and monotonic timers |
| [Test helpers](test_support/README.md) | Deterministic scripts and pinned upstream fixtures |

Public Rust contracts have companion Markdown files for language-independent
explanations. Specialized lifecycle, receive, probe, drain and scheduling code
lives under `stream/session/` rather than a single catch-all implementation.

## Resource and concurrency policy

One owner polls a driver; adapters arrange readiness wakeups. The parent clock
registers the earliest pending deadline. Connections and streams share finite
budgets; serialized-byte reservations follow queued frames through partial
writes. Configuration preserves the full 65,536-byte payload and 65,564-byte
protected frame. Overflow is backpressure or an explicit resource failure,
never normal completion. There are no invented wire flow-control frames.

Whole-frame round-robin output preserves per-stream order, OPEN allocation
order and GOAWAY barriers. Independent consumer polling and per-stream byte
limits isolate slow consumers. Standalone Session defaults are finite; composed
sessions inherit the configured instance-wide budgets. Driver policy also caps
registrations at 256 and pending probes at 64 per connection. Listener and
undelivered connection-state counts are bounded by the connection policy.

`tracing` events use module targets and structured function context. Composition
configures its subscriber, filters and stderr destination; protocol code does
not install a global subscriber. Traces omit raw application bytes, peer reason
text, tokens and secret material. Normal CLI status output remains on stdout,
with runtime/usage failures on stderr.

## Build, exercise and run

From the repository root:

```sh
cargo build --locked
cargo test --locked
cargo test --locked modules::transport::connection::tests::acceptance
cargo test --locked modules::transport::adapters
cargo test --locked modules::transport::driver
cargo doc --locked --no-deps
cargo run --locked -- --help
cargo run --locked -- help serve
cargo run --locked -- serve
```

The final command intentionally exits 1 until real module integration and normal
server composition are supplied. There is no fake-authentication switch.
Generated API documentation is under `target/doc/talksphere/`.

## Evidence and integration handoff

Behavior and fixtures use [CR2SE revision
8e848eb2e70ef7cd4490a753f9e647fdddcd99fd](https://github.com/beothorn/CR2SE/tree/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport).
The [acceptance matrix](ACCEPTANCE.md) maps TR-01–TR-15 to executable tests and
explicitly identifies real-crypto assertions still pending. The [Transport
plan](../../../plans/1_Transport_module/PLAN.md) records each step's validation.
[Roadmap steps 3–5](../../../plans/PLAN.md) implement real primitives/credentials,
verify cryptographic vectors and mutations, and demonstrate two local nodes.

Two language-independent findings remain documented: [diagnostic BOM handling](../../../CR2SE%20bugs/0001-diagnostic-byte-order-mark.md)
and [key derivation order](../../../CR2SE%20bugs/0002-key-derivation-before-auth-verification.md).
The implementation follows the normative specification in both cases.
