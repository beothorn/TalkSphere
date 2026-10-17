# Complete frame I/O

The connection driver uses one FrameReader and FrameWriter per socket. These
helpers depend on the Socket port and header codec; they do not own authentication,
socket admission or consumer dispatch. They implement the byte boundaries of
TR-02/TR-14 at [CR2SE revision 8e848eb](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/pseudocode/README.md#2-exact-socket-io).

Reads validate the complete header before allocating a bounded body, preserve
progress across Pending and never read past the current frame. A rejected frame
permanently disables the reader; the owner must close the socket. EOF at a
boundary returns None; every nonempty incomplete prefix is Truncated. An owner
must still fail unfinished streams on boundary EOF. Integrity phase is explicit
per direction and cannot change during a frame. Output is always unverified.

The writer owns at most one complete frame, rejects replacement while busy and
retains offsets across Pending. Zero progress fails; Interrupted retries. Both
directions yield and wake their owner after 64 immediate operations so a ready
socket cannot monopolize a poll. No bytes or tokens are logged.

Contracts: [module](mod.md), [reader](reader.md), [writer](writer.md).
From the repository root:

```sh
cargo build --locked
cargo test --locked modules::transport::framing
cargo test --locked
cargo run --locked -- --help
```

Networking remains unavailable through `serve`; exercise framing through the
polling API and deterministic tests. Actual tag verification is step 9.
