# Transport codecs

Step 3 implements the 12-byte version 1 header in `header.rs`, the closed frame
type registry in `frame_type.rs` and structured `HeaderError` failures in
`error.rs`. Header processing performs no socket I/O or payload allocation.
Step 4 adds [control payload codecs](control/README.md) for PING/PONG, GOAWAY
and ERROR/CANCEL, with bounded encoding and allocation-free borrowed decoding.
Step 6 adds [HELLO/AUTH formats](handshake.md), validating exact lengths and
version/algorithm selectors without authenticating the peer.

Behavior follows CR2SE revision
[`8e848eb2e70ef7cd4490a753f9e647fdddcd99fd`](https://github.com/beothorn/CR2SE/tree/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd),
specifically [wire sections 2–4 and 12](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/README.md),
[header pseudocode section 3](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/pseudocode/README.md#3-header-encoding-and-preliminary-validation)
and [acceptance case TR-03](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/acceptanceTests/README.md).
The [pinned fixtures](../test_support/fixtures/README.md) provide independent
PING/PONG header bytes; DATA expectations follow the wire layout directly.

## API and validation

`FrameHeader::new(FrameType, stream_id, payload_len)` validates the outgoing
fields. Private fields prevent later mutation into an invalid header.
`encode()` returns a fixed `[u8; 12]` with version 1, zero flags and big-endian
integers. `decode(&[u8])` requires exactly 12 bytes and returns the same immutable
header type, exposing `frame_type()`, `stream_id()` and `payload_len()`.

Decoding rejects unsupported versions, nonzero flags, payload lengths above
65,536 and every unassigned frame type, preserving the rejected values in
typed errors. It also rejects nonzero IDs for HELLO/AUTH/PING/PONG/GOAWAY and
zero IDs for OPEN/DATA/END/CANCEL. ERROR permits either scope; whether a
nonzero ID exists is a later connection-state check.

`frame_len(FrameProtection::Untagged)` and `frame_len(FrameProtection::Tagged)`
calculate header plus payload, with an additional 16 bytes only for Tagged,
using checked arithmetic and checked conversion to the platform size type.
At the maximum payload these sizes are 65,548 and 65,564 bytes. Tag state must
come from the direction's handshake state; selecting Tagged proves nothing
about authentication. Payload length always excludes the header and tag.

Header acceptance is preliminary. The control codecs validate their own payload
shapes; other payload shapes (including nonempty DATA and empty END), handshake
sequencing, stream existence/parity/allocation, terminal
states and integrity validation are later steps. Keeping payload/state checks
separate preserves their required error scope. A decoded header must not be
treated as authenticated data or dispatched to an application.

The decoder never consumes a payload or scans for another boundary after an
error. A future reader must stop on rejection before allocating/reading the
claimed payload and close the connection without resynchronizing. Unsupported
version errors are distinct because the driver must close without encoding a
response. Actual shutdown/error-response behavior and complete TR-03 acceptance
coverage remain pending until the connection driver exists.

## Build, run and test

From the repository root:

```sh
cargo build --locked
cargo test --locked modules::transport::codec
cargo test --locked --test control_payloads
cargo test --locked --doc
cargo test --locked
cargo run --locked -- --help
cargo run --locked -- help serve
```

The public API example on `FrameHeader` is exercised by documentation tests.
Unit tests check upstream bytes, exact DATA sizes 1 and 65,536, every version,
type and nonzero flag value, stream-ID combinations, oversized lengths and
short/trailing headers. There is no codec CLI command; `serve` still exits 1
because networking is unimplemented. See the [Transport README](../README.md)
and [plan](../../../../plans/1_Transport_module/PLAN.md) for subsequent work.
