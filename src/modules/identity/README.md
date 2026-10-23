# Identity

Status: public contracts and resource policy are implemented; identity creation,
protected storage and operations remain pending the [plan](../../../plans/3_Identity_module/PLAN.md).
No scaffold returns fake credentials. This is a direct in-process boundary.

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

There is no Identity CLI command yet. `cargo run --locked -- serve` returns status 1
until Identity and normal server composition exist. No listener is opened.
Production coverage follows [the shared procedure](../../../docs/coverage.md).

[Test helpers](test_support/README.md) pin the reviewed upstream fixtures and
provide ordered dependency scripts and restart/failure substitutes.

Cryptography now exposes canonical public signing-key validation for Identity;
ID derivation and the Identity-facing provider remain subsequent steps.

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
and immutable snapshots; durable contact persistence remains a later step.

[Local capabilities](authority.md) bind an issuing domain, identity, purpose,
lifetime and byte budget; revocation does not erase durable credentials.

Cryptography supplies [protected identity material](../cryptography/identity_keys.md)
for guarded platform import/storage; Identity does not inspect private primitive fields.
