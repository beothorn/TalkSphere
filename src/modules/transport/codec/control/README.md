# Control payload codecs

Transport step 4 implements PING/PONG tokens, GOAWAY fields and the shared
ERROR/CANCEL diagnostic format. The future Transport reader/writer calls these
codecs after checking headers and, on protected connections, integrity. They
depend only on standard byte/text operations and the shared `tracing` facade;
they do not own sockets, timers, authentication, stream state or error routing.

The reference remains CR2SE revision
[`8e848eb2e70ef7cd4490a753f9e647fdddcd99fd`](https://github.com/beothorn/CR2SE/tree/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd),
particularly [wire sections 2, 10 and 11](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/README.md),
[payload pseudocode section 10](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/pseudocode/README.md#10-payload-codecs)
and [TR-11/12/13 acceptance cases](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/acceptanceTests/README.md).

## Contracts and organization

- [PingToken](ping.md) retains exactly eight opaque bytes for either PING or PONG.
- [ErrorPayload](diagnostic.md) borrows a valid UTF-8 reason and a nonzero code
  for ERROR or CANCEL; the private shared diagnostic codec also serves GOAWAY.
- [GoAway](goaway.md) retains a full-width last-accepted ID, any registered code
  including NO_ERROR, and a borrowed reason.
- [ErrorCode](error_code.md) defines all eleven wire values; unknown values fail.
- [PayloadError](error.md) distinguishes malformed lengths, text and codes.
- [Module exports](mod.md) describe the public boundary and byte limit.

Constructors validate outgoing values; decoders require complete payloads with
no trailing data and borrow text without allocating. Encoders produce exact
big-endian fields and bounded output (at most 1,028 bytes for ERROR/CANCEL,
1,032 for GOAWAY). Reasons are limited to 1,024 UTF-8 bytes, not characters;
invalid UTF-8 and a leading byte-order mark are rejected. Interior Unicode
characters are preserved without normalization. The BOM check follows the
normative specification; [bug 0001](../../../../../CR2SE%20bugs/0001-diagnostic-byte-order-mark.md)
records its omission from the pseudocode.

Reasons never determine machine state. Callers must supply secret-free outgoing
text and safely handle untrusted incoming text. Payload types deliberately do
not derive Debug; trace events describe function flow without tokens or reasons.
Logging uses module targets and a `function` field. Application composition
owns subscriber/filter configuration; these library codecs install no subscriber,
write nothing to stdout/stderr themselves and add no CLI logging option.
The current CLI does not invoke these codecs or expose a runtime logging setup.

Parsing GOAWAY does not check parity, greatest-opened IDs, repeated limits or
initiate draining. Parsing ERROR/CANCEL does not select stream/connection error
scope. PING/PONG matching, replies and timeouts also remain future steps. Tests
cover payload cases from TR-11/12/13, not complete connection acceptance.

## Build, run and test

From the repository root:

```sh
cargo build --locked
cargo test --locked modules::transport::codec::control
cargo test --locked --test control_payloads
cargo test --locked --doc
cargo test --locked
cargo run --locked -- --help
cargo run --locked -- help serve
```

The integration tests compose headers with payloads and capture trace events
without network I/O. Unit tests use the pinned PING/PONG tokens and independent
error/shutdown byte examples, exhaust the numeric code registry, check byte
bounds and reject truncation, trailing bytes, invalid text and forbidden codes.
See [coverage instructions](../../../../../docs/coverage.md) for production
measurement. `serve` remains unavailable and exits 1; no runtime behavior is
added by these codecs. See the [Transport plan](../../../../../plans/1_Transport_module/PLAN.md).
