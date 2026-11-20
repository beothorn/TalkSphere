# Storage

Contract foundations for [CR2SE Storage version 1](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/README.md).
Implementation is in progress under the [38-step plan](../../../plans/9_Storage_module/PLAN.md).
There is currently no registered paid Storage provider or runnable Storage command.

Services will call requester/provider contracts and coordinate Ledger decisions.
Storage owns immutable byte leases, durable effects and results; Board owns
publication and Peer Protocol handles authenticated exchanges. Storage consumes
injected clock, random/hash and local durability ports, never another family's
runtime or another owner's private database. Application encryption and recovery
catalogs remain requester responsibilities. One lease is one copy.

From the repository root:

```sh
cargo build --locked
cargo test --locked modules::storage
cargo run --locked -- --help
```

See [requirements and port audit](REQUIREMENTS.md) and the plain-language
[contract](contract.md). No-argument application invocation shows help; use
`cargo run --locked -- help <topic>` for detailed existing command help.

Test-only deterministic ports model wall-clock rollback, collisions, entropy and
hashing failure, bounded partial reads and concurrent durable writes. Restart
reconstructs an adapter from retained bytes; lost replies preserve commitment.
These helpers supply no production authentication or durability.

Pinned complete definitions, terms, hash and literal prices are recorded in
[fixture provenance](fixtures/README.md); these pass independent schema/hash
checks and do not establish provider conformance.

[Exact pricing](pricing.md) implements single-round store and usage calculations
with a documented overflow argument; it does not register a Storage provider.

[Offering validation](terms.md) rejects invalid current economic terms and
unrepresentable prices at advertised maxima before use. Steps 1–16 and 18–27 are
complete; durable store creation and full handler/network integration remain pending.
[Operation validation](operation.md) checks complete values and separate byte challenges.

[Lease authorization](lease.md) binds access to trusted requester/provider identities.

[Retention helpers](retention.md) protect lease duration against clock corrections
and reject unverified reboot timing; [requester clock policy](clock_policy.md)
checks returned acceptance times against an explicit local skew bound. Durable
anchor/lease persistence remains part of the later owner implementation.

[Quota admission](quota.md) atomically bounds bytes, lease slots, pending operations
and retained results, including per-owner pending work. Durable owner recovery
must rebuild its holds before exposing new admission.

The Linux [byte adapter](bytes/mod.md) durably stages exact uploads and retains
immutable positional range handles. Exercise it with
`cargo test --locked modules::storage::bytes` from the repository root.
Byte installation alone does not create an active paid lease.

[Metadata images](image.md) retain exact lease descriptions, operation results,
preparation tokens and retention anchors with identity/version/checksum checks.
The byte owner also implements [atomic metadata persistence](bytes/snapshot.md).
`cargo test --locked modules::storage::image` exercises actual metadata/byte reopen
and gates missing or corrupt active objects before recovery admission.

The [participant journal](journal.md) durably stages original-key completions,
prepares exact effects and recovers original tokens after uncertain writes.
`cargo test --locked modules::storage::journal` covers concurrent preparation,
dependency failures and actual protected snapshot reopen. No paid provider is
registered until the remaining owner, operation and integration steps are ready.

[Exact metadata effects](effect.md) and the participant journal now apply
explicit coordinator commit/abort decisions once, preserving original tokens
and guarded lease mutations. Complete handlers, payment integration and executable network Storage remain
steps 17–38.

The Linux [recovery gate](recovery/mod.md) keeps new work closed until original
Services coordination, retention recovery, active byte integrity and quota
reconstruction complete. Receiving/unreferenced inventory and unavailable
coordinator history fail closed. Exercise it with
`cargo test --locked modules::storage::recovery`.

[Store assessment](assessment.md) validates complete/unresolved input before
upload execution and computes exact work prices from local dimensions. Services
keeps that metadata separate from provider wire factors.
`cargo test --locked modules::storage::assessment` exercises metadata and capacity
refusal; `cargo test --locked metadata_pricing` verifies the Services callback.

Paid lease creation awaits [upstream acceptance-time clarification #11](https://github.com/beothorn/CR2SE/issues/11),
recorded in the [protocol evidence](../../../CR2SE%20bugs/0008-storage-acceptance-time-before-durability.md).
A slow durable commit currently exposes conflicting timestamp requirements.
Step 17 remains unchecked and no paid provider is advertised.

Independent [requester store validation](requester/store.md) and
[acknowledged catalog retention](requester/catalog.md) complete step 18 while
step 17 remains pending #11. Run
`cargo test --locked modules::storage::requester` from the repository root.
These contracts do not establish a complete paid provider or choose a new
acceptance-time rule.

Independent [retrieval acceptance](retrieve.md) completes step 19 with scoped
immutable [range references](bytes/range.md). Timely pins survive later expiry,
removal and unlink; new expired/foreign/invalid-range calls fail before data reads.
Run `cargo test --locked modules::storage::retrieve` for actual byte-adapter tests.
[Complete range results](retrieve_result.md) now finish step 20 with pre-I/O
resource bounds, bounded whole-object integrity scans, and original-key durable
result retention through the participant journal. Corrupt or short backing bytes
produce no successful output; retained local results survive source loss and
journal reopen. This local recovery adds no remote retry operation. Complete
paid network integration remains a later step.

[Requester retrieval verification](requester/retrieve.md) checks original scope,
range, count, whole-object hash metadata and exact current price. Full reads are
rehashed; partial reads compare optional private original bytes and explicitly
report when only metadata was checked. Exercise step 21 with
`cargo test --locked modules::storage::requester::retrieve`.

[Renewal assessment](renewal.md) checks authenticated ownership, active retention,
expected-expiry guards, current duration/total limits and exact size-based price
without extending the lease. Requesters compare the sole size factor with their
retained lease. Exercise step 22 with
`cargo test --locked modules::storage::renewal`.

[Accepted renewal staging](renewal_commit.md) completes step 23 through recovered
owner admission and existing guarded participant preparation/commit. Competing
renewals cannot both prepare; failed writes and lost acknowledgements retain
original decision semantics. [Requester renewal validation](requester/renew.md)
checks the exact previous/new expiration and preserves immutable lease metadata.
Run `cargo test --locked modules::storage::renewal_commit` and
`cargo test --locked modules::storage::requester::renew` from the repository root.

[Logical removal](removal.md) completes step 24 through exact current pricing,
original-key result retention and guarded participant commitment. Later operations
see an inactive lease; accepted retrieval pins still complete. Independent
[requester validation](requester/remove.md) checks `removed:true`, ID and price
without changing original store charges or claiming physical erasure. Run
`cargo test --locked modules::storage::removal` and
`cargo test --locked modules::storage::requester::remove` from the repository root.

[Bounded terminal cleanup](recovery/cleanup.md) completes step 25 with persisted
expiry, shared immutable handle lifetimes, protected deletion and capacity
reconstruction. Follow the returned cursor through each sweep to avoid a pinned
object blocking later IDs. History and results remain retained; cleanup does not
promise secure erasure or unlimited history capacity. Run
`cargo test --locked modules::storage::recovery::cleanup` from the repository root.

[Paid byte evidence](byte_check.md) completes step 26 with current positive check
pricing, authenticated active ownership, exact one-byte pins and explicit optional
overlap/one-second interval rules. Expected bytes remain private and outcomes are
requester-owned. Run `cargo test --locked modules::storage::byte_check` from the
repository root; handler and paid network composition remain later steps.

[Requester byte verification](requester/byte_check.md) completes step 27 using
private original bytes and original send/response/end observations. Wrong timely
bytes fail; late, incomplete, foreign or expiration-ambiguous evidence is
inconclusive. No outcome automatically refunds charges or changes trust. Optional
[secure sampling](requester/sampling.md) uses bounded rejection draws without bias.
Run `cargo test --locked modules::storage::requester::byte_check` and
`cargo test --locked modules::storage::requester::sampling` from the repository root.

The independent [standard catalogue](standard.md) now supplies the pinned complete
definition and checks Storage schema meaning through public Services Standards.
This is partial progress on step 28: real handler installation and usable provider
publication remain pending step 17 and issue #11. Exercise it with
`cargo test --locked modules::storage::standard` from the repository root.

The [installed Storage requester](requester/client/mod.md) now connects exact
scoped quote validation, ordinary complete-result checks and separately paid
byte-check rules through actual Services traits. Original local lease/range
expectations are keyed by provider and original attempt, preserving colliding
opaque IDs and delayed results. Run
`cargo test --locked modules::storage::requester::client` from the repository root.
This advances step 28; provider creation/publication still depends on step 17.

[Requester Peer Protocol integration](requester/client/PEER.md) now exercises
ordinary quotes, exact consent, bounded detached bodies, terminal result gates
and local requester coordination/accounting through actual public module ports.
Paid byte-check proposals preserve private expectations. Wanted invitations use
actual requester-owned Board publication, local input/spending policy and finite
issuance; withdrawal/refusal cannot spend credits. Payment tests cover provider
currency, requester issuance and independent signed external-issuer settlement
with exact fees and one-time confirmation. Run
`cargo test --locked modules::storage::requester::client::tests::wire`.
Ordinary remote-reply tests use volatile adapters. Separate
[requester native recovery tests](requester/client/RECOVERY.md) reopen independent
Services/Ledger files at coordination boundaries and after lost durability replies,
preserving exact retained results and one charge. Native payment tests also retain
finite issuance and third-party payment state through requester/issuer reopen.
Authenticated provider processes
and full Storage participant recovery remain separate pending requirements.
