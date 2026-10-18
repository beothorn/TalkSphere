# Transport

Transport owns CR2SE connections, authentication, frames and multiplexed
streams. It consumes Identity, Cryptography, TCP and timers through explicit
boundaries; it does not own service dispatch, balances or application retries.

**Status:** steps 1–12 are implemented: public contracts, replaceable dependency
interfaces, local configuration validation, handle ownership checks and
deterministic test helpers, frame/control codecs and bounded complete-frame I/O.
`transport::start()` still returns the typed local error
`TransportError::NotImplemented`, before any I/O. There is no driver, TCP
adapter yet; [stream routing](stream/README.md) now provides registration and
OPEN dispatch for authenticated composition.

Frame I/O is independently exercised with `cargo test --locked modules::transport::framing`;
see the [framing guide](framing/README.md). The [integrity layer](integrity/README.md) authenticates frames through injected
cryptographic primitives and enforces independent directional sequence counters.
Role-bound [authentication constructions](handshake/README.md) request signatures
and key derivation through the injected ports; real primitives remain pending.

## Layout

- `mod.rs`: module exports and the unavailable networking entry point.
- `api.rs`: control requests, acceptance results, events and terminal outcomes.
- `handshake/`: role-bound authentication and handshake sequencing.
- `integrity/`: protected frames, directional counters and replay rejection.
- `config.rs`: immutable validated configuration and resource-limit types.
- `handles.rs`: opaque connection, listener and stream capabilities.
- `error.rs`: machine-readable local request failures.
- `codec/`: version 1 headers, control/error payloads, registries and structured validation
  errors; see the [codec guide](codec/README.md).
- `ports/io.rs`: replaceable sockets, listeners, connection attempts and clock.
- `ports/identity.rs`: public credentials and Transport-only signing authority.
- `ports/crypto.rs`: primitive operations and opaque transient-secret types.
- `ports/mod.rs`: dependency bundle and structured adapter failures.
- `test_support/`: test-only scripted dependencies, recording consumer and
  pinned upstream fixtures; see the [helper guide](test_support/README.md).

Introduce focused codec, handshake, stream and adapter files/subfolders during
implementation. Keep session state private and test dependency substitutes
separate from usable runtime capabilities; `test_support` is compiled only with
`#[cfg(test)]` and is not part of the application or public library interface.

## Public boundary

`api::TransportControl` defines the future driver's `submit` and `poll_event`
calls; no implementation of this trait is supplied yet. Requests cover
connect/listen/accept, protocol registration, stream operations and connection
control. `Accepted` reports local admission; subsequent `Event` values carry
authenticated context, verified bytes, half-close, terminal results and ping
outcomes. A queued write does not prove delivery or application success.

`TransportConfig::new` rejects zero handshake timeouts, zero connection/stream
counts and byte budgets smaller than a full supported frame. An outbound
budget counts serialized bytes and needs at least 65,564 bytes; per-stream
incoming buffering needs at least 65,536 payload bytes. These are TalkSphere
configuration rules that preserve CR2SE's frame-size support, not new wire
negotiation. Admission and queue enforcement remain later steps.

`ConnectRequest::new` rejects an outgoing port of zero, while listen requests
may use zero for an OS-selected port. `ExpectedPeer::Known` carries the required
expected ID for a configured contact; `Unpinned` still requires a successful
handshake. Other request validation, including protocol selection, belongs to
the planned runtime/codec steps; constructing a request does not accept it.

`TransportError` describes immediate local rejection. Terminal `Failure`
values distinguish a local decision, explicit peer failure and dependency
failure, with machine-readable codes. Stream/connection scope comes from the
event's handle or ID; these types do not define wire error encodings.

## Authority and lifetimes

The control port belongs to trusted local composition, never a network parser.
A peer's public `IdentityId` carries no local signing or stream authority.
`ConnectionId` identifies an instance for observations, while `ConnectionHandle`
grants control over it. A consumer's `StreamHandle` exposes only the former.
Stream requests also require the matching `StreamOwner` granted by successful
registration. Handles have private fields and no constructors from wire IDs or
peer data; the future driver will issue them after admission.

`StreamHandle::authorize` checks both connection instance and owner before a
stream effect. Sharing a capability clone deliberately delegates that same
authority. Runtime liveness, registration and state still require checks by
the future driver; this method alone does not prove a connection is active.

Instance equality uses private reference-counted tokens. They are process-local
and cannot be serialized or restored after restart; an IPC adapter would need
its own authenticated scoped-handle mapping. A new connection cannot reuse an
old handle even when the wire stream number is the same.

`Identity::sign_authentication` accepts only an `AuthenticationMessage` that
Transport constructs; it exposes no unrestricted signing API or private keys.
Cryptography's associated secret types have no Clone/Debug requirement and
must erase/release their material when dropped. These adapter obligations and
real cryptographic correctness are not proven by this interface-only step.

## I/O and concurrency choice

Use a single owner per Transport driver with nonblocking `Poll`/`Context`
interfaces from the Rust standard library. Socket reads/writes may make partial
progress, and pending operations must arrange a wakeup. The injected monotonic
clock registers the next deadline, allowing later tests to advance time without
sleeping. No executor, TCP implementation or cryptographic crate is selected
by these contracts.

Composition supplies the `Dependencies` bundle: socket provider, clock, one
purpose-scoped local Identity capability and a Cryptography adapter. The driver
will serialize frame writes and deliver queued events outside internal locks;
other tasks can exchange bounded messages with it. This is a Rust integration
choice, not a CR2SE concurrency or deployment requirement.

## Protocol references

Initial baseline:
[`8e848eb2e70ef7cd4490a753f9e647fdddcd99fd`](https://github.com/beothorn/CR2SE/tree/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd).

- [Wire specification](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/README.md)
- [Inputs](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/inputs/README.md)
  and [outputs](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/outputs/README.md)
- [Pseudocode](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/pseudocode/README.md),
  [wire vectors](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/pseudocode/TestVectors.md)
  and [acceptance cases TR-01–TR-15](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/acceptanceTests/README.md)

See the [Transport plan](../../../plans/1_Transport_module/PLAN.md) for the
remaining implementation steps and the [main roadmap](../../../plans/PLAN.md)
steps 3–5 for real Identity/Cryptography integration. No acceptance case is
claimed as fully passed: codecs cover TR-03 header rules and payload cases from
TR-11/12/13, while delivery, authentication and TCP closure still need a runtime.
The control parser follows the specification's BOM prohibition; see
[bug 0001](../../../CR2SE%20bugs/0001-diagnostic-byte-order-mark.md) for the
corresponding pseudocode omission.

## Build, run and test

From the repository root:

```sh
cargo build --locked
cargo test --locked modules::transport
cargo test --locked modules::transport::test_support
cargo test --locked modules::transport::codec
cargo test --locked --test control_payloads
cargo test --locked --doc
cargo doc --locked --no-deps
cargo run --locked -- help serve
cargo run --locked -- serve
```

The final command currently reports the missing Transport implementation on
stderr and exits with status 1. It starts no listener. Run all tests with
`cargo test --locked`; build and run details are also in the
[project README](../../../README.md).

Unit tests exercise configuration limits, endpoint validation and handle scope;
documentation tests check the public configuration example and reject forged
control/signing capabilities. Helper tests additionally check partial/coalesced
I/O, EOF offsets, manual time, wakeups, exact dependency scripts, resource
release and consumer event order. Codec tests use upstream PING/PONG headers
and independent expected DATA bytes, payload boundaries and malformed headers.
They do not prove the pending Transport
state machine or real cryptographic behavior. Generated API documentation is under
`target/doc/talksphere/modules/transport/index.html` after `cargo doc`.
