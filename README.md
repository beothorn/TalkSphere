# TalkSphere

TalkSphere is a Rust server implementation and proof of concept for
[CR2SE](https://github.com/beothorn/CR2SE), developed module by module. The intended
model is ad-free social features supported by shared resources and paid services.
The server will also have client capabilities for testing and advanced users;
separate UI clients remain future work.

**Current status:** Transport, Cryptography, Identity and the authenticated server
integration milestone are implemented. On Linux, `serve` loads protected existing
keys, listens, authenticates configured peers and handles controlled shutdown.
Frame integrity protects against modification and connection-local replay;
**frame payloads remain visible**. Application encryption is a separate capability.

Production registers no application protocol yet, so unsupported OPENs are refused.
The [Board module](src/modules/board/README.md) now implements direct-call
publication, protected persistence, attributed caches, selection and document tools.
The Ledger direct-call module now supports durable finite accounting, settlement,
transfers and encrypted claims with protected Linux persistence and local inspection.
The [Services library](src/modules/services/README.md) now supports typed handler
registration, exact quotes and durable local coordination, with
[acceptance evidence](src/modules/services/ACCEPTANCE.md) over real Board/Ledger
contracts. Peer Protocol and paid Storage remain later milestones.
This is not yet the minimum useful node or full-feature PoC. See the
[roadmap](plans/PLAN.md), [Peer Protocol plan](plans/8_Peer_Protocol_module/PLAN.md), [Services plan](plans/7_Services_module/PLAN.md),
[Ledger plan](plans/6_Ledger_module/PLAN.md),
[Board plan](plans/5_Board_module/PLAN.md),
[integration plan](plans/4_Transport_integration/PLAN.md)
and [executed acceptance evidence](docs/transport-integration/ACCEPTANCE.md).

## Build and test

Use stable Rust with Cargo, rustfmt and Clippy; the project uses edition 2024.
From the repository root:

```sh
cargo build --locked
cargo test --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo doc --locked --no-deps
```

Cargo.lock is tracked. For an optimized executable, run `cargo build --locked --release`.
See [production coverage](docs/coverage.md) for the measured-code exclusions and
coverage command. Follow [AGENTS.md](AGENTS.md) when contributing.

Focused checks:

```sh
cargo test --locked composition::server
cargo test --locked --test server_peers
cargo test --locked --test server_streams
cargo test --locked --test server_runtime
cargo test --locked --test server_trace
```

The [Transport](src/modules/transport/README.md),
[Cryptography](src/modules/cryptography/README.md) and
[Identity](src/modules/identity/README.md) guides document module contracts,
fixture revisions and additional tests. Real socket/process tests use isolated
stores and assigned ports; controlled tests cover fragmentation and extreme counters.

## Initialize and inspect keys

```sh
./target/debug/talksphere identity-init --identity-store ./node-a
./target/debug/talksphere identity-init --identity-store ./node-b
./target/debug/talksphere identity-show --identity-store ./node-b
```

Initialization is explicit and refuses existing credentials. Inspection never
replaces missing/corrupt keys. Both commands print only the public canonical ID.
Run inspection before starting the server: the protected store has one exclusive
writer lock, retained throughout runtime. Linux directories/files are owner-only
0700/0600; records and backups are **not encrypted at rest**. Secure their location
and copies. Key backup/import library operations require separate privileged grants;
they are not exposed as CLI commands. Application recovery catalogs remain separate.

## Connect two nodes

In terminal B, start the already initialized node:

```sh
./target/debug/talksphere serve --identity-store ./node-b --listen 127.0.0.1:7002
```

Wait for `ready <B_PUBLIC_ID> 127.0.0.1:7002`. In terminal A, replace
`B_PUBLIC_ID` below with that complete `cr2se:...` ID:

```sh
./target/debug/talksphere serve --identity-store ./node-a --listen 127.0.0.1:7001 \
  --contact 'B_PUBLIC_ID?address=127.0.0.1&port=7002'
```

Both terminals report `authenticated <PUBLIC_ID> <Initiator|Acceptor>` only after
proof verification and session derivation. Reverse the roles to connect B to A.
Each startup contact is attempted once; failed admission/connection does not create
an implicit reconnect loop or stop the listener. ID-only contacts cannot be resolved
without Discovery. Explicit contacts and `--stored-contacts yes` cannot be combined;
a selected stored snapshot must already exist and contain usable IP/port endpoints.

SIGINT/SIGTERM stop admission and drain accepted work to a finite deadline.
`stopped` means owned resources have been released. Repeated termination requests
force remaining connections closed. Runtime failure or forced timeout exits 1;
invalid syntax exits 2; normal shutdown and help exit 0. No arguments show help.
Transient work is not replayed or resumed after restart.

## Help, logging and policy

```sh
./target/debug/talksphere --help
./target/debug/talksphere help serve
./target/debug/talksphere help --identity-store
./target/debug/talksphere help --listen
./target/debug/talksphere help --version
./target/debug/talksphere help logging
```

Every option spelling has detailed help through `help <topic>`. `serve --help`
also works. The default listener is loopback port zero; readiness reports its
actual assigned port. All other addresses require explicit configuration.

| Option | Default and units |
| --- | --- |
| `--identity-store` | Required existing protected directory for serve |
| `--listen` | `127.0.0.1:0` |
| `--contact` | None; repeat up to 64 endpoint-bearing contacts |
| `--stored-contacts` | `no`; `yes` selects the protected snapshot |
| `--max-connections` | 64 simultaneous connections |
| `--max-streams` | 128 per connection |
| `--outbound-bytes` | 4196096 serialized bytes per Transport instance |
| `--buffered-bytes` | 65536 verified payload bytes per stream |
| `--handshake-ms` | 10000 milliseconds |
| `--shutdown-ms` | 5000 milliseconds |

Policy must support full protocol frames and positive representable deadlines.
Operational logs use stderr, with `RUST_LOG=info` by default. For diagnosis use
`RUST_LOG=talksphere=trace`, target filters, or `RUST_LOG=off`. Intentional public
CLI results/status use stdout; neither stream contains private keys or payloads.
Help/version do not require a valid logging filter. The application version is
not a CR2SE conformance claim.

## Organization and handoff

Protocol domains live under `src/modules/<module>/`; the thin main delegates to
`src/cli/`. Real adapters and runtime policy/lifecycle live under `src/composition/`.
See the [server guide](src/composition/server/README.md) for contract links.
Individual protocol findings and confirmed upstream issue URLs are retained in
[CR2SE bugs](CR2SE%20bugs/README.md). References always identify GitHub sources.

The reviewed behavior baseline is CR2SE d40b6278; older fixtures retain their
original provenance. Real process tests use TalkSphere at both ends and do not
establish interoperability with an independent implementation. The next roadmap
module is Peer Protocol; paid Storage, minimum-profile services and paid recovery remain later work.

## Board document tools

Validate a public document with `cargo run --locked -- board-validate --board-file
board.json`. This checks the Board format and exact prices; it does not establish
handler availability or validate separately supplied definitions. Inspect an existing
protected Linux snapshot with `cargo run --locked -- board-show --board-store
board-state --publisher CR2SE_ID`. The publisher must match the stored identity.
Use `talksphere help --board-file` or `talksphere help --board-store` for limits,
output and failure rules. These commands do not publish or open peer connections.

## Ledger inspection

Inspect an existing protected Linux Ledger through
`cargo run --locked -- ledger-show --ledger-store DIRECTORY --local-identity CR2SE_ID --account CR2SE_ID`.
All options are required in this order. Output keeps owned/issued currencies
separate and shows exact available amounts and participant/payment status counts.
Inspection acquires the exclusive writer lock and never initializes or repairs
accounting. Use `cargo run --locked -- help ledger-show` or help with any option
spelling for defaults and errors. Ledger mutations currently use its authorized
library contracts; network controls await Peer Protocol composition.

Run `cargo run --locked --example ledger` for the bounded volatile accounting
walkthrough, and see the [Ledger evidence](src/modules/ledger/ACCEPTANCE.md) for
all local payment modes, failure/replay tests and later composition obligations.

## Local Services exercise

Run `cargo run --locked --example services` for an explicitly installed custom text
handler using real Board/Ledger contracts: it prints `price=7, remaining=93`.
Use `cargo run --locked -- help services` or
`cargo run --locked --example services -- --help` for details;
`cargo test --locked --example services` checks the complete example.
This demonstration uses volatile stores and starts fresh on every run; it opens
no network connection. Protected-store recovery is exercised independently by
`cargo test --locked modules::services::recovery_tests`. See the
[Services guide](src/modules/services/README.md) for ownership and recovery rules.
