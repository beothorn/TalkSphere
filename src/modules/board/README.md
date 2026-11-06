# Board

Board's direct-call module is implemented: bounded lossless documents, exact
terms, shared Services schema validation, complete prepared publications, atomic
immutable local snapshots, Linux durability/recovery, attributed remote caches
and explicit role-aware selection. All 24 implementation steps are recorded in
[the plan](../../../plans/5_Board_module/PLAN.md); executed rows and deferred
integration are in [acceptance evidence](ACCEPTANCE.md).

Board is called by trusted local administration, future Services and Peer Control.
Composition supplies one coordinated publication owner per publisher, grants,
platform storage and actual provider/requester capabilities. Identity supplies
public ID/text contracts; Services supplies schema formats without requiring its
runtime. Board does not own sockets, keys, balances or accepted invocation journals.
This keeps document validation independent from network, execution and payment.

[Upstream Board](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/board)
is the pinned behavior source. Unknown optional metadata and original numeric
spellings are preserved; all calculations use exact integer digits. Unknown
mandatory semantics remain unusable. Duplicate decoded JSON members are rejected.
[Issues #8 and #9](../../../CR2SE%20bugs/README.md) remain open: normative duplicate
and nonempty-precondition rules are enforced, while namespace grammar awaits
upstream clarification. Unknown names never become satisfied automatically.

## Build, test and exercise

Run from the repository root:

```sh
cargo build --locked
cargo test --locked modules::board
cargo test --locked modules::services
cargo test --locked --test board_cli
cargo test --locked --test board_persistence
cargo test --locked --doc
cargo run --locked -- --help
```

The persistence integration suite is Linux-only. It uses real protected stores
and a separate-process reopen; deterministic cut points are not physical power
failure testing. No serialized local Board adapter or independent peer
interoperability is claimed.

Create and validate a public empty Board without publishing it:

```sh
cat > board.json <<'JSON'
{"version":1,"providedServices":[],"wantedServices":[]}
JSON
cargo run --locked -- board-validate --board-file board.json
cargo run --locked -- help --board-file
```

Validation reports format status separately from definition completeness and
local publishability. To inspect a store created by authorized library composition:

```sh
cargo run --locked -- board-show --board-store board-state --publisher CR2SE_ID
cargo run --locked -- help --board-store
```

Replace CR2SE_ID with its exact expected publisher. Inspection requires an existing
owner-only Linux directory, takes the writer lock, validates stored Board/definition
associations and prints original public Board JSON. Corruption and identity mismatch
fail without replacing data. Every command/option spelling has detailed help.

## Local publication and recovery

[Provider](state.md) implements direct local publication/read/definition/selection.
Its API documentation includes an executable empty-publication example.
[Prepared](prepared.md) validates a complete candidate, including matching
[definitions](definition.md), [semantic capabilities](semantics.md) and exact
[response bounds](response.md), before state changes. Provided offerings require
actual provider capability; wanted offerings require requester codecs and authorized
input. Descriptions and installation hints confer no permission.

[Authority](authority.md) grants publisher/action/byte-scoped local access;
[Context and Snapshot](contract.md) document deadlines, cancellation and immutable
pins. New owners have no publication until explicitly committed; reads return
Missing rather than manufacturing an empty Board. Local schema version 1 remains
separate from opaque snapshot references.

[Limits](config.md) documents defaults and local refusal policy, including the
1048576-byte document budget, depth 31 before wrapping, 1024 definitions, 16 MiB
aggregate source bytes, 64 cached associations, 128 retained revisions and 300-second
cache lifetime. Full wrapped responses must fit the peer body's fixed 1048576 bytes
and depth 32. Snapshot limits include retained pins; release old pins to admit more
revisions. Strict rejection of extra definitions and conservative logical contract
comparison are local publication policy.

Provider.new is explicitly volatile. Provider.open uses the declared
[Persistence](persistence.md) port and revalidates loaded state before exposure.
[Linux FileStore](linux.md) provides no-follow traversal, 0700/0600 owner protection,
exclusive writer lock, atomic replacement and file/directory synchronization.
Public documents are permission-protected, not encrypted; this is separate from
Identity secrets and the paid Storage service. Coordinating the same publisher
across multiple machines or stores remains deployment responsibility.

A known precommit failure leaves prior state intact. A possibly committed write
returns Unknown and blocks current reads/new publication until explicit recover
loads durable state; it never automatically repeats the write. Old pins and copied
terms stay immutable. Recovering storage does not resume an application invocation.

[Remote Cache](cache.md) keeps direct and forwarded provenance distinct, with
finite freshness and no hidden network refresh. [Definition association](cache_definitions.md)
rejects delayed replies against replaced receipts. [Selection](selection.md)
assigns requester/provider from provided/wanted direction, preserves the exact
issuer and rejects changed or removed terms. Quote acceptance, actual charges and
durable accepted obligations are deferred to Services/Peer Protocol; the BD-09
integration rows remain explicitly deferred.

Production CLI registers no fake Board/service protocol and has no publication
command bypassing absent handlers. Completing Board's library and tools does not
complete a minimum useful node. Ledger is the next roadmap module.
