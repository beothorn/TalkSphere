# Configure and exercise Peer Protocol

Peer Protocol is an opt-in library composed through public Transport, Board,
Services and Ledger contracts. `serve` retains its authenticated Transport-only
profile. Paid Storage and the minimum useful node milestone remain on the main
roadmap; a custom Echo example is not a minimum-profile or optional-family claim.

## Build, run and help

From the repository root on Linux with loopback networking available:

```sh
cargo build --locked
cargo run --locked -- --help
cargo run --locked -- help peer-protocol
cargo run --locked --example peer -- --help
cargo run --locked --example peer -- help run
cargo run --locked --example peer -- run
cargo test --locked --example peer
```

No example arguments show help successfully. `--help`, `-h`, `help`,
`help --help` and `help -h` show its overview; `help run` explains every runtime
default and side effect. `run` explicitly creates fresh protected temporary
identities and ephemeral loopback listeners, pins their public IDs, retrieves
Board and definition metadata, validates an exact seven-credit quote, obtains
separate local spending consent and completes custom inline text work.

Expected intentional stdout:

```text
Peer Protocol: authenticated nodes=2, price=7, requester remaining=93, provider remaining=93
```

The example uses actual crypto, authentication and production handlers with
independent owners in one process. It shares the independently installed custom
`org.example.echo` contract with the local Services example. It supports only
small inline text work; detached bodies, wanted roles, paid/included checks,
three-party settlement and persistent issuer process restart have the separate
[real-node exercises](NODES.md). It implements no standard paid family.

Help/success exits0, invalid arguments exit2, and runtime/configuration failures
exit1 on stderr. Non-Linux execution returns an explicit unsupported-platform
failure. Help does not initialize logging, keys, stores or sockets.

```sh
RUST_LOG=warn,talksphere::modules::peer_protocol=trace cargo run --locked --example peer -- run
```

Operational logs go to stderr; stdout is reserved for the result. The default
filter is `info`; invalid filters fail before networking. Target/level filters
make worker/stream transitions visible without recording application bodies,
claim material or private keys. Frame integrity authenticates transport bytes;
application payload confidentiality remains a separate module capability.

## Explicit composition order

1. Initialize Identity storage only through authorized local administration; load existing protected credentials for normal startup and pin expected contacts.
2. Open independent identity-bound Ledger and Services stores under their separate local authority domains; initialize absent stores only when explicitly authorized.
3. Install independently understood family requester/provider implementations into Registry before publication; retrieved definitions cannot install code.
4. Validate and atomically publish Board offerings/definitions using that Registry, explicit issuer/pricing and supported remote-check rules.
5. Construct the actual Ledger accounting participant and Services coordinator, resolving retained original decisions before admitting new paid work.
6. Build Endpoint with the exact local identity, public owners, separate authorities, read-only admission gate and explicit transfer-fee policy.
7. Install protected claim decryption, wanted input policy and quote consent only when locally authorized; absent capabilities return unsupported.
8. Register Bridge with the real Transport control and the incoming Endpoint factory, retaining unique authenticated connection scope and finite lifetimes.
9. Poll Transport events into Bridge, then poll Bridge even on turns without application events, using a shared wake source for readiness and worker completion.
10. Prepare each outgoing request as a one-shot Endpoint request plan, open its factory on the intended authenticated connection and await its bounded completion observation.

The [example owners](../../examples/peer/owners.rs) and
[reactor composition](../../examples/peer/node.rs) show these boundaries in actual
Rust without moving sockets or private accounting maps into protocol internals.
The [endpoint contracts](../../src/modules/peer_protocol/engine/endpoint/mod.md)
and [request plans](../../src/modules/peer_protocol/engine/endpoint/requests/mod.md)
explain inputs, outputs, authority and failure rules without requiring Rust knowledge.

## Finite defaults and separate budgets

| Boundary | Example policy |
| --- | --- |
| Bridge |4 connections/exchanges,2 workers,131072 pending input bytes/stream,1048581 queued output bytes/stream,32000000 aggregate framing-reservation bytes,32 controls/second/stream,10-second original exchange lifetime |
| Wire logical values |65536 bytes/value,4096 nodes,16 detached references,4 unaccepted quotes and4 exchanges |
| Endpoint aggregate admission |268435456 logical bytes,65536 body references,4 handlers/authenticated peer across all connections |
| Services/Ledger owners |Finite module defaults; separate identity-bound authority, journals, holds and permanent replay records |
| Example family |One separately authorized seven-credit custom operation with inline text bounded to128 result bytes |

Endpoint reservations acquire capacity before handler effects and remain until
worker-owned cleanup; Transport separately bounds framed input/output and worker
admission. Logical admission is not an RSS limit. Keep production handlers'
allocations and durable resources bounded independently. A control-only issuer
may publish an empty Board while enabling only authenticated Ledger controls.

## Storage and recovery limits

The example owns randomly named mode0700 temporary Identity directories, which it
removes after its node owners drop. Its service handler, Services journal and
Ledger books are explicitly volatile: each run starts from fresh100-credit demo
accounts and cannot resume. It never opens an operator store. Production
composition must retain protected Identity, Board, Ledger, Services and each
family's participant resources in separate independently locked stores.

On restart, reload exact identity-bound records, restore every participant and
resume original prepared/committed decisions through Services recovery before
new admission. Retained terminal/replay history is not evictable to make room for
fresh financial work. Do not reinitialize a missing/corrupt store, rerun an unknown
handler, infer remote rollback from cancellation or trust a forwarded receipt.
A committed ordinary response may require application-specific recovery;
explicit cached retries apply only to the installed family's authorized operation.

Work completion and third-party payment are separate. The requester submits a
real owner-signed transfer with separate exact fee consent; the provider obtains
its own opaque, normally completed observation over an independent original-issuer
connection before settling its local book. Unreachable/refused/changed-fee payment
keeps already committed work unsettled/unknown. Included-check delivery also
retains uncertain guards; local END admission is not remote receipt. The open
[paid-check timeout question](https://github.com/beothorn/CR2SE/issues/10) remains
a limit on global charging claims, not permission for refunds or new wire messages.
