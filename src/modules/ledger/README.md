# Ledger

Ledger implements local issuer-separated accounting, trust, finite budgets,
reservations, durable participant settlement, signed transfers and encrypted secret
claims for [CR2SE](https://github.com/beothorn/CR2SE). The direct-call module and
protected Linux persistence are complete under the [33-step plan](../../../plans/6_Ledger_module/PLAN.md).
See [requirements](REQUIREMENTS.md) and [acceptance evidence](ACCEPTANCE.md) for
pinned sources, executed assertions and remaining integration boundaries.

## Build, run and test from the repository root

```sh
cargo build --locked
cargo run --locked -- --help
cargo run --locked --example ledger
cargo test --locked modules::ledger
cargo test --locked --test ledger_cli --test ledger_persistence
cargo doc --locked --no-deps
```

The example uses explicitly volatile storage and illustrative opaque public IDs;
it creates no authenticated peers, files, keys or network requests. It demonstrates
balance 100/available 93 after reserve 7 and balance 93 after commit plus exact replay.
For durable use, trusted composition opens a Linux `FileStore` with positive limits,
then explicitly initializes an absent Ledger with finite budgets or opens existing
state. Only administrator authority can initialize, grant/revoke, reconcile owned
balances, change trust or create/invalidate claims. No public CLI initializes funds.

## Local inspection

```sh
cargo run --locked -- ledger-show --ledger-store DIRECTORY --local-identity CR2SE_ID --account CR2SE_ID
cargo run --locked -- help ledger-show
cargo run --locked -- help --ledger-store
```

All three options are required in the displayed order. Select an existing owner-only
Linux store, its expected local identity and one external issuer/owner account.
Output is exact decimal amounts, trust in millionths and aggregate participant/payment
status counts. It never prints bearer claims, keys, accepted terms or result references.
Inspection takes the exclusive writer lock; stop that store's writer first. Missing,
corrupt, insecure, busy or unsupported-platform stores fail with status1 and diagnostics
on stderr; invalid syntax exits2; successful output/help exits0. Inspection does not
initialize, repair, grant credits or contact a peer. Every option spelling has detailed
`help <topic>`; help ignores malformed logging configuration.

## Callers, dependencies and boundaries

Services supplies authorized accepted operation keys and immutable terms, validates
complete results and records coordinator decisions before commit/abort delivery.
Peer Protocol supplies authenticated caller/connection/stream context and maps exact
control records onto Ledger ports. Trusted composition owns action-scoped Authority
issuers; possession is privileged, never granted from peer payload fields. Separate
Evidence authority is required to assert an independently authenticated original issuer
response. Identity supplies protected scoped signing/decryption and Cryptography owns
strict signature/envelope algorithms. Public adapters borrow handles; no raw keys are
exposed. These dependencies use contracts, not private state or cross-module locks.

Ledger owns its accounting/replay database. Local platform persistence works without
paid Storage. Family service effects and the Services coordinator remain separate;
Ledger's prepared participant does not own their database or execute service work.
[Actual Services coordination](../services/ACCEPTANCE.md) now exercises this public
participant across all eight fault cuts and a protected-store subprocess restart.
Network quote/transfer/claim exchanges and standalone module IPC remain later composition. This module does not establish a minimum useful
node, cross-peer atomic payment, independent interoperability or physical power-loss
validation. The next roadmap module is Peer Protocol.

## Exact accounting, authority and limits

Balances use exact uint64 per issuer, with owned-by-issuer and issued-to-owner views
kept distinct. Remote balance observations never silently reconcile local accounting.
Self work consumes a separate finite internal budget without self-balances. Trust is
an exact local millionths value 0..1000000; unknown is zero. Authentication, credits
and claim redemption never raise it. Providers explicitly accept requester credits
and third-party payment risk. Grants consume finite issuance; revocation does not
replenish lifetime issuance authority. Reservations protect both spending and incoming
uint64 headroom against all competing changes.

Local default limits are 4096 accounts per view/operations, 65536 transfer/claim records,
64 live quotes and 16777216 state bytes. These are local policy, not peer wire limits.
Configure deliberately before opening; smaller incompatible limits refuse existing
state. All operation bindings are conservatively retained until a coordinator retention
proof exists; transfers and terminal claims retain replay guards for Ledger lifetime.
Maintenance removes only zero balance entries; exhaustion refuses new admission.

## Persistence and recovery

The Linux store uses descriptor-anchored no-follow traversal, owner-only 0700 directory,
regular 0600 single-link files and one exclusive writer. Creation is explicit. One
complete identity-bound private image is staged, file-synchronized, renamed atomically
and directory-synchronized before commitment acknowledgement. The schema/version and
all accounting/role/effect/replay invariants are validated on load. Corruption is not
absence and never generates a replacement Ledger. Permissions protect against other
users, not the same user or root. Multiple nodes sharing keys require one coordinated
identity writer; copying keys/stores does not replicate a budget safely.

Known precommit failure preserves prior state. Unknown commitment gates affected reads
and mutations until authorized recovery reloads the possibly committed image. Reuse
the original operation/creation key and query status; never re-execute work or repeat
issuance under a new key merely because a reply was lost. Prepared holds survive
cancellation and deadlines until an authorized recovered coordinator decides. Abort
cannot undo committed effects. Exact commit/transfer/redemption retries return original
results without another accounting effect.

Third-party service completion remains distinct from Pending/Unknown/Unsettled/Settled
payment. Only exact original issuer evidence resolves payment; a requester-forwarded
receipt does not. Unknown/refused payments conservatively retain holds pending explicit
resolution. Payment action requests describe submission/query through composition and
never open sockets or repeat service execution.

## Secret claims and diagnostics

Portable claims are secret bearer material. Guarded imports/exports and decrypted
plaintext erase their owned buffers; callers must also protect original input and
private delivery channels. Creation stores only SHA256 commitment, amount, restrictions
and lifetime state, reserving finite issuance. Losing an acknowledged export cannot
recover its secret: recover/query the creation key and explicitly invalidate as needed.
Redemption uses the generic identity envelope, checks exact owner/issuer/domain,
compares commitment in constant time and uses the stored amount. Same-owner retry is
idempotent; another owner cannot reuse it. Invalidation releases only unredeemed holds.

Structured TRACE events explain operations without keys, claims, raw terms or payloads;
composition configures logging and stderr filters. Resource/dependency failures remain
typed and secret-free. The acceptance matrix records actual fault/race evidence and
separates controlled dependency substitutes from real cryptographic/filesystem checks.
