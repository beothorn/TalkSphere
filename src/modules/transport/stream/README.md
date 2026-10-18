# Streams

This domain owns per-connection stream numbering, authorized protocol
registration, queued OPEN dispatch and bounded bidirectional DATA; lifecycle
state follows in later steps. It depends on the TCP role contract;
framing and cryptography remain separate responsibilities. The connection
owner serializes mutations and retains only high-water marks for terminal IDs.

Behavior follows [CR2SE sections 6–8 and TR-07](https://github.com/beothorn/CR2SE/tree/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport).
See [ID contracts](ids.md) and the [Transport plan](../../../../plans/1_Transport_module/PLAN.md).

From the repository root, build with `cargo build --locked`, exercise with
`cargo test --locked modules::transport::stream`, and inspect application status
with `cargo run --locked -- help serve`. Runtime networking remains pending.

See [registration](registry.md) and [session routing](session.md) for caller
authority and event contracts. DATA immediately following OPEN is supported without an OPEN acknowledgement.
Tests concatenate interleaved stream bytes and exercise full-size payloads,
invalid empty sends, authority rejection and bounded queue overflow.
