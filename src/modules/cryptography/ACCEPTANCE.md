# Cryptography acceptance evidence

Baseline: [CR2SE 80f6795864ecf060e23482ee42909da4a649ecf0](https://github.com/beothorn/CR2SE/tree/80f6795864ecf060e23482ee42909da4a649ecf0),
[CR-01–CR-11](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/cryptography/acceptanceTests/README.md)
and [AT-C01–C04](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/AcceptanceTests.md).
This is the required Cryptography module with a direct in-process provider and
real Transport adapter; optional service implementations and node conformance
are not claimed. Fixture provenance is in [test support](test_support/README.md).

Run from the repository root:

```sh
cargo test --locked modules::cryptography
cargo test --locked composition::cryptography
cargo test --locked --test cryptography_privacy
cargo test --locked --doc
```

## Cryptography suite

| Case | Executed assertions and source | Scope or caveat |
| --- | --- | --- |
| CR-01 | [hash](primitives/hash.rs): literal empty/abc digests, every abc partition including single-byte and empty delivery; byte limits and atomic failed updates | Finalization consumes state; reuse also has a compile-fail example. |
| CR-02 | [signature](primitives/signature.rs), [real handshake](../../composition/cryptography/tests.rs): both exact AUTH signatures verify; message/domain/signature/public-key changes fail | Ordinary Ed25519, no prehash or native serialization; strict weak-point policy is explicitly local pending bug 0003. |
| CR-03 | [keys](primitives/keys.rs), [credential](credential.rs), [decryption round trips](envelope/decrypt.rs), [audit](tests/acceptance.rs): independent key requests, exact binding signature/domain bytes, ID/key substitution, every binding-message/credential byte mutation and 63/65-byte signatures rejected | Encryption validates before consuming entropy. Credential validation is stateless, not a persistent key-selection cache. |
| CR-04 | [derivation](envelope/derivation.rs), [HKDF](primitives/kdf.rs), [header](envelope/header.rs), [AEAD](primitives/aead.rs), [encryption](envelope/encrypt.rs), [decryption](envelope/decrypt.rs): context, salt, PRK, key, header, sealed bytes and complete object match independent literals; fixed decryption recovers exact plaintext | Generic API has no external AAD parameter. Expected values are stored upstream literals, not production-generated round trips. |
| CR-05 | [decryption](envelope/decrypt.rs): fixed local and distinct generated recipient both round-trip empty and UTF-8 bytes; [encryption](envelope/encrypt.rs) and header tests check exact lengths/recipient fields | Self-encryption is the same recipient operation. No Identity persistence is inferred. |
| CR-06 | [decryption](envelope/decrypt.rs), [AEAD](primitives/aead.rs): every header/ciphertext/tag byte flip, wrong local ID/public/private key, nonce and AAD fail with no plaintext result | APIs return complete owned plaintext or error, never provisional chunks. |
| CR-07 | [header](envelope/header.rs), [object](envelope/object.rs), [public-boundary audit](tests/acceptance.rs): unsupported selectors, every truncation, below-tag lengths, U64_MAX/overflow, budgets and trailing bytes rejected before derived material/body allocation | Complete-buffer API; a header-first size function supports bounded stream callers. No incremental plaintext interface exists. Missing final tags never return plaintext. |
| CR-08 | [agreement](primitives/agreement.rs), [adapter failures](../../composition/cryptography/tests.rs): wrong sizes and low-order inputs producing all-zero results fail; no raw result is used as an AEAD key | The selected native multiplication has no fallible backend operation for 32-byte inputs, so injected native primitive errors are not applicable. Fallible entropy and adapter error mapping are separately exercised; no fake primitive-success claim. |
| CR-09 | [lifecycle](envelope/tests/lifecycle.rs), [secrets](secrets.rs), [randomness](random.rs), [privacy integration](../../../tests/cryptography_privacy.rs): fresh scripted key/nonce pairs, distinct objects, both decrypt, releases after success/failure/abandonment, partial entropy failure and secret-free diagnostics | Synchronous operations have no mid-call cancellation API. Ownership release/zeroizing storage is tested; physical erasure of all compiler/backend copies is not claimed. |
| CR-10 | [separation](envelope/tests/separation.rs): independently valid Messaging payload rejected as generic; opaque storage/retrieval/decryption succeeds without intermediary private access; [Transport tags](primitives/integrity.rs) and real frame tests preserve visible payloads | Return types/docs promise no sender identity or Transport confidentiality. Messaging fixture is locally generated from the pinned construction with independent Python/libsodium, not an upstream-supplied vector. |
| CR-11 | [separation](envelope/tests/separation.rs): swapped independently protected objects still authenticate; signed test-application manifest binds positions/lengths/hashes and rejects altered manifest, reordered or missing objects | This manifest is a test application scenario, not a new CR2SE chunk/manifest format or production shared-key chunk protocol. |

## Common boundary cases

| Case | Evidence | Nonapplicable assertions |
| --- | --- | --- |
| AT-C01 | [Provider tests](provider/tests.rs), real adapter tests and audit authority test exercise direct declared ports and exact bytes; key-purpose and consumption compile-fail examples prevent generic capability misuse | No serialized adapter, IPC authority service or hostile linked-code sandbox is advertised. Local signing requires private authority; public credentials cannot produce signatures for the corresponding identity. |
| AT-C02 | Config/primitive/envelope boundaries test exact and outside budgets; synthetic lengths check overflow without large allocation; entropy failure propagates without synthetic success or hidden retry | No durable commitment, remote invocation or unknown remote outcome exists in this synchronous primitive module. |
| AT-C03 | Provider snapshot test mutates caller plaintext after encryption without changing returned object; consumed capability examples and observed releases enforce lifetimes | No mutable publication, durable obligations or restart/replay store is owned here. Identity persistence and Transport replay state belong to their modules. |
| AT-C04 | Malformed/truncated complete-object and lifecycle tests return no partial success; privacy capture checks trace fields; provider has no network or optional-module dependency | No progress stream, billing evidence, refunds or mid-call cancellation protocol is advertised. |

## Pseudocode section 18 and integration handoff

The executable matrix covers exact key binding, widths, independent generation,
all-zero agreement, context/labels, unsigned lengths, empty objects, full-header
AAD, tampering, recipient mismatch, fresh per-object material, self-encryption and
opaque forwarding. Integrity and content keys have distinct capability purposes
and distinct tested constructions. Private material has no wire serialization.

Installing the same persistent pair on multiple nodes and detecting conflicting
cached bindings for one ID require the later Identity/persistence boundary; they
are not passed by this stateless Cryptography module. General shared-key chunking,
manifest formats and confidential interactive protocols require service-specific
specifications and are not implemented or advertised here. Real Identity and full
two-node Transport integration remain roadmap steps 4–5.

## Protocol findings

- [0003](../../../CR2SE%20bugs/0003-ed25519-weak-point-validation.md): exact weak-point fixture demonstrates unspecified Ed25519 acceptance; local strict rejection is documented, not silently promoted to a normative rule.
- [0004](../../../CR2SE%20bugs/0004-secret-cleanup-guards.md): published cleanup guards miss failures before try/finally; scoped cleanup follows the normative lifecycle requirement and has no wire change.

These open upstream issues limit unconditional interoperability claims. Passing
this suite is implementation evidence, not a security audit or full node conformance.

## Completion validation

Step 22 passes the repository's formatting, Clippy (`-D warnings`), locked tests,
build and overview/detailed help checks, plus generated API documentation and the
expected unavailable `serve` behavior. The complete suite contains 227 passing
unit/integration/documentation tests, including an executable Provider example.
[Production coverage](../../../docs/coverage.md) is **95.23% (3570/3749 lines)**
with test modules, helpers and test-only observer fields excluded. All 22 planned
Cryptography steps are complete within the scope and open findings recorded above.

## Identity dependency update: signature profile

Identity steps 3–4 adopt revision d40b6278's explicit Ed25519 verification profile.
[Public-key validation](primitives/verification_key.rs) and
[profile regressions](primitives/tests/signature_profile.rs) check canonical
encoding, all small-order points, published negative A/R/S rows, collection
rejection and mixed-order cases separating exact from cofactored equations.
Existing honest AUTH/binding vectors remain required. The historical CR-02
local-policy caveat above is superseded for this boundary; no batch API or
process-separated adapter is claimed. Newly expanded cleanup cases involving
persistent Identity storage remain in the Identity lifecycle plan.
