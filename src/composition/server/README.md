# Authenticated server composition

Status: all 24 integration steps are implemented; final verification is recorded
in the [plan](../../../plans/4_Transport_integration/PLAN.md) and
[acceptance matrix](../../../docs/transport-integration/ACCEPTANCE.md).
The direct Linux runtime owns protected Identity authority, real Cryptography,
TCP/readiness, monotonic timers and the Transport driver. CLI/trusted library
callers supply configuration; Transport retains wire/session/stream state.
Application protocols and service coordination belong to later modules.

| Contract | Responsibility |
| --- | --- |
| [Configuration](config.md) | Effect-free policy, contacts, finite representable deadlines |
| [Lifecycle](mod.md) | Stages and secret-free failure categories |
| [Credentials](credentials.md) | Existing protected keys, read/sign grants, writer lock |
| [Transport composition](transport.md) | Real adapters, shared readiness and actual listener binding |
| [Contacts](contacts.md) | One-shot pinned connection attempts |
| [Event pump](pump.md) | Bounded polling and retained readiness notifications |
| [Routing](routing.md) | Trusted registration and isolated consumer ownership |
| [Shutdown](shutdown.md) | Stop admission, drain, force policy and process signals |
| [Runtime](run.md) | Executable lifecycle and public status observations |

From the repository root:

```sh
cargo build --locked
cargo test --locked composition::server
cargo test --locked --test server_runtime --test server_peers --test server_streams
cargo test --locked --test server_trace
cargo run --locked -- --help
cargo run --locked -- help serve
```

Follow the root README's explicit identity initialization and two-terminal commands.
serve never generates replacement keys. Its default bind is loopback with an assigned
port; an explicit contact requires an IP/port and expected identity. Contacts are
not retried automatically. Inspection shares the protected store's exclusive lock.
SIGINT/SIGTERM drain to --shutdown-ms; repeated requests force remaining work closed.
Normal Stopped is reported after driver/key ownership drops. Runtime failure exits 1,
syntax failure exits 2. Unsupported platforms fail without an unprotected fallback.

Production registers no application protocol. Tests register bounded isolated
consumers through trusted composition and never enable a fake service/debug CLI.
The runtime protects frame integrity, not payload confidentiality, and never treats
Transport admission as service completion, spending authorization or remote receipt.

Fixtures control only the boundaries under test. Native real_wire tests pin exact
upstream bytes with real primitives; separate executable peers use production
randomness. Test sockets, manual time, extreme-counter fixtures and wake probes
are excluded from production coverage. The supported local binding is direct;
network peers are independent processes, not serialized local module deployment.
