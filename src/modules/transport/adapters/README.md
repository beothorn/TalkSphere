# Platform adapters

`TcpProvider` implements nonblocking connect/listen/accept and byte-stream
socket operations. Mio owns OS readiness; a shared worker wakes the polling
Transport driver. `SystemClock` registers its earliest monotonic deadline with
that worker. Adapter construction binds no sockets. Dropping attempts, sockets,
listeners and timers releases their resources and retained wakers.

The [Mio readiness contract](https://docs.rs/mio/1.2.4/mio/struct.Poll.html)
requires retrying operations after spurious readiness and continuing I/O until
WouldBlock. Adapters arm task wakers before I/O; the bounded connection driver
self-wakes when it yields before exhausting ready work. All socket access goes
through Mio. Only `net` and `os-poll` features are enabled.

[Driver composition](../driver/mod.md) checks shared admission limits before
allocating handshake state. No adapter implements identity or cryptography.
The normal CLI remains unavailable until real module capabilities are supplied.

From the repository root:

```sh
cargo build --locked
cargo test --locked modules::transport::adapters
cargo test --locked modules::transport::driver
cargo run --locked -- help serve
```

Loopback tests use OS-assigned ports, test actual readiness/timer wakeups, byte
exchange, closure, bind conflict and refused connection. They make no claim of
cryptographic interoperability. Protocol baseline remains [CR2SE revision
8e848eb](https://github.com/beothorn/CR2SE/tree/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport).
