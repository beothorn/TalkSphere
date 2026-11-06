# Identity

Status: all 30 planned steps are implemented and verified for the direct in-process
binding and protected Linux filesystem adapter. See the [plan](../../../plans/3_Identity_module/PLAN.md)
and [acceptance matrix](ACCEPTANCE.md). Server composition is implemented in roadmap step 5.

Identity owns persistent credentials, scoped authority and configured contacts.
It calls Cryptography for algorithms and a protected platform store for durability;
Transport owns sessions and fresh proof transcripts. Ledger owns trust and balances,
and the application owns its recovery catalog. No Discovery or paid Storage runtime
is needed. Composition lives outside protocol internals.

Reviewed baseline: [CR2SE d40b6278](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/identity),
including the [inputs](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/identity/inputs/README.md),
[outputs](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/identity/outputs/README.md),
[pseudocode](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/identity/pseudocode/README.md)
and [acceptance cases](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/identity/acceptanceTests/README.md).
See [API](api.md), [limits](config.md), [errors](error.md) and [ports](ports.md).

From the repository root:

```sh
cargo build --locked
cargo test --locked modules::identity
cargo test --locked --doc
cargo doc --locked --no-deps
cargo run --locked -- --help
cargo run --locked -- help serve
```

Explicit identity-init and identity-show commands require --identity-store DIRECTORY.
Configured serve loads existing keys and opens the selected listener on Linux;
serve without the required store option exits 2. See the root README for commands.
Production coverage follows [the shared procedure](../../../docs/coverage.md).

[Test helpers](test_support/README.md) pin the reviewed upstream fixtures and
provide ordered dependency scripts and restart/failure substitutes.

Cryptography now exposes canonical public signing-key validation for Identity;
ID derivation and the protected Identity-facing provider use that validation.

[Binary identity operations](id.md) derive the pinned IDs and validate public-key
relations through the [real Cryptography adapter](crypto.md), without claiming possession.

[Canonical text formatting](text.md) emits exact RFC 4648 Base32 identities and
matches both published textual vectors. Strict parsing accepts only the specified
ASCII case variations; normalization never repairs invalid input.

[Contact formatting](contact.md) emits ID-only or canonical IP/port QR payload
text, including percent-encoded IPv6; no QR image or network call is produced.

Contact parsing accepts either query order, decodes once and rejects malformed
fields or non-IP endpoints within an explicit input budget.

[Configured contacts](contacts.md) support bounded authorized in-memory updates
and immutable snapshots; the Linux store persists complete snapshots atomically in a separate record.

[Local capabilities](authority.md) bind an issuing domain, identity, purpose,
lifetime and byte budget; revocation does not erase durable credentials.

Cryptography supplies [protected identity material](../cryptography/identity_keys.md)
for guarded platform import/storage; Identity does not inspect private primitive fields.

[Local credential records](record.md) stage both keys and verify their cached
public association; records are not durable until the protected store commits them.

[Protected storage](storage/README.md) now supports owner-only Linux reads and
exclusive writer locking and atomic complete credential installation. Other platforms
return Unsupported. OS access controls protect against other users, not the same account/root.

The [Identity provider](provider.md) composes authorized creation from independent
Cryptography keys and the protected store, returning no usable identity before commit.

Loading validates the complete protected association without key generation or repair.
Run `cargo test --locked --test identity_persistence` for real Linux restart evidence.

[Selection policy](selection.md) validates exactly one configured identity and
rejects transparent signing/encryption-key changes. Bounded recipient pins reject
competing valid bindings. Key copies do not copy service state, and the local
writer lock cannot coordinate independent machines.

[Scoped signing](operations.md) signs exact caller-defined protocol messages through
loaded handles. Transport and other consuming protocols retain responsibility for
transcripts, role/domain separation, freshness and authentication state.

[Proof verification](proof.md) validates the claimed ID/key relation before the exact
signature. It grants no trust and leaves freshness, replay and session state to callers.

[Scoped decryption](operations.md) explicitly selects generic envelopes and returns
only fully authenticated guarded plaintext; service-specific formats are unsupported.

[Protected backup](recovery.md) snapshots both pairs to an independently protected
store through Backup authority, without returning private bytes or overwriting an
existing destination. The local format is permission-protected, not encrypted.

[Validated restore](recovery.md) requires explicit identity-bound Import authority,
validates the complete backup association and refuses to overwrite existing keys.

The [lifecycle audit](LIFECYCLE.md) maps secret acquisition/release and durable restart
boundaries to tests and describes synchronous cancellation and native backend limits.

The [Transport adapter](../../composition/identity.md) owns loaded scoped
Identity authority and retains the protected writer lock. Tests use persisted and
reloaded credentials with real Cryptography for both handshake roles and pinned
AUTH/frame vectors; denied signing and contact mismatches prevent session publication.

The [acceptance matrix](ACCEPTANCE.md) maps ID-01–ID-10 and applicable shared cases
to executed tests, reviewed revisions, supported adapters and scope limits.
