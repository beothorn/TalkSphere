# Connection driver

This owner composes the tested framing, handshake, integrity and stream domains
around one socket. A parent driver supplies shared Identity/Cryptography,
protocol registrations, monotonic time and timer wakeups. The socket owns
readiness wakeups. Application callbacks run outside connection mutation.

The driver follows [CR2SE Transport](https://github.com/beothorn/CR2SE/tree/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport)
with strict scripted dependencies in tests. It does not provide real credentials
or advertise an authenticated server. See [public operations](mod.md).

From the repository root:

```sh
cargo build --locked
cargo test --locked modules::transport::connection
cargo run --locked -- help serve
```

Tests cover coalesced handshakes in both roles, instance continuity, deadlines,
EOF/truncation, failed output, error-response eligibility and secret/socket
release. Real cryptographic erasure and interoperability remain integration work.
