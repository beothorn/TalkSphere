# Cryptography module plan

Status: proposed for review; no implementation steps are complete.

Cryptography is the next required module in the [main roadmap](../PLAN.md),
where it is step 3; its module folder number is 2 because setup is not a module.
It needs no running CR2SE module and supplies the real primitives needed by
Transport and the later Identity implementation.

Every implementation step includes focused unit tests, failure cases, explanatory
comments, safe entry tracing and updates to affected public-contract Markdown,
README and help documentation as required by [AGENTS.md](../../AGENTS.md).
The final audit consolidates evidence rather than postponing tests.

## Steps

- [ ] **1.** Define Cryptography’s public contracts and dependency boundaries.
- [ ] **2.** Add deterministic dependency helpers and pinned cryptographic fixtures.
- [ ] **3.** Add protected secret types with explicit ownership and cleanup.
- [ ] **4.** Connect production key generation to secure platform randomness.
- [ ] **5.** Implement SHA-256 hashing for bytes and ordered chunks.
- [ ] **6.** Implement independent Ed25519 and X25519 key generation.
- [ ] **7.** Implement ordinary Ed25519 signing and verification.
- [ ] **8.** Implement X25519 agreement with all-zero rejection.
- [ ] **9.** Implement bounded HKDF-SHA-256 extraction and expansion.
- [ ] **10.** Implement authenticated encryption without provisional plaintext release.
- [ ] **11.** Implement the Transport integrity-tag construction.
- [ ] **12.** Implement encryption-key binding and recipient credential validation.
- [ ] **13.** Implement the generic identity-envelope header codec.
- [ ] **14.** Validate complete encrypted objects against resource limits.
- [ ] **15.** Implement the exact identity-envelope key derivation.
- [ ] **16.** Implement encryption for a validated recipient.
- [ ] **17.** Implement authenticated decryption for the intended local identity.
- [ ] **18.** Verify secret cleanup and diagnostic privacy across composed operations.
- [ ] **19.** Adapt real Cryptography operations to Transport’s dependency port.
- [ ] **20.** Test envelope separation and independent protected objects.
- [ ] **21.** Audit Cryptography against all upstream acceptance cases.
- [ ] **22.** Document Cryptography coverage and the Identity integration handoff.

## Reference baseline and scope

Reviewed against CR2SE revision
[`80f6795864ecf060e23482ee42909da4a649ecf0`](https://github.com/beothorn/CR2SE/tree/80f6795864ecf060e23482ee42909da4a649ecf0).
Recheck upstream before implementation and record any adopted revision changes.

- [Cryptography specification](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/cryptography/README.md), [inputs](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/cryptography/inputs/README.md) and [outputs](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/cryptography/outputs/README.md).
- [Pseudocode and fixed envelope vector](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/cryptography/pseudocode/README.md), [CR-01–CR-11](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/cryptography/acceptanceTests/README.md) and [shared acceptance guide](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/AcceptanceTests.md).
- [Module boundaries](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/ModuleContracts.md), [Identity ID formula](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/identity/README.md), [Transport construction](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/transport/README.md) and [Transport vectors](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/transport/pseudocode/TestVectors.md).

Use established algorithm libraries; choose and document supported versions and
features during implementation rather than implementing cryptographic algorithms.
Organize code by responsibility under `src/modules/cryptography/`, for example
primitives, secrets, envelope codecs and test support, creating folders as needed.

This plan covers the required Cryptography contract, a bounded single-object
envelope and a real Transport crypto adapter. Persistent credential storage,
contact/text formats, service-specific envelopes, production manifest/chunk
protocols and a running authenticated server remain later roadmap work.
Test-owned Identity credentials enable vector checks without implementing Identity.

Both previous [protocol bug reports](../../CR2SE%20bugs/README.md) are fixed in
this baseline. Transport’s historical fixture baseline remains recorded separately;
this planning review does not claim its implementation was revalidated against
every upstream change. No additional confirmed protocol defect was identified
in this review; investigate contradictions during implementation and report them
with reproducing evidence rather than silently inventing wire behavior.

## Step details

### 1. Public contracts and boundaries

Define typed requests, results, errors, resource limits, protected key capabilities and an injectable entropy boundary under `src/modules/cryptography/`. Distinguish malformed input, authentication failure, denied key access and unavailable dependencies without exposing secrets. Test invalid configuration and capability boundaries with the scaffold.

Cryptography owns primitives, generic identity envelopes and transient secrets; Identity owns persistent credential storage, recovery and contacts. Use the shared version-1 ID formula without a running Identity module. Transport retains transcripts, directional sequencing and replay state. Keep composition adapters outside primitive internals and document direct in-process support without claiming process separation.

### 2. Test helpers and fixtures

Add test-only scripted entropy, fault injection, protected-handle lifecycle observation and literal fixtures from the pinned envelope and Transport vectors. Record source URLs, revision and expected lengths; never derive expected bytes using the implementation under test. Test helper exhaustion and failure behavior. Deterministic private keys and constructors must remain unavailable in ordinary production configuration.

### 3. Secret ownership

Introduce purpose-specific signing, persistent and ephemeral agreement, shared-secret, PRK and derived-key capabilities, adding concrete backend storage as subsequent steps need it. Prevent accidental public serialization, diagnostic exposure and signing/agreement interchange. Select established zeroization support and document its guarantees and limitations.

Test release on ordinary drop, failed construction and early return; instrument lifecycle behavior without claiming a unit test proves physical memory erasure. Keep persistent key-store format and atomic credential installation for Identity.

### 4. Secure entropy

Add a fallible platform CSPRNG adapter with bounded requests and no deterministic fallback. Document dependency/version choices and commit the lockfile when adding crates. Test fresh independent requests, unavailable entropy and partially filled output cleanup with the injected source; add a production adapter smoke test without probabilistic uniqueness assertions. Key generation in later steps must use this boundary.

### 5. Hashing

Use an established implementation for exact bytes and incremental ordered input, with documented limits and finalization behavior. Check empty and abc known answers and multiple chunk partitions (CR-01), including empty chunks and invalid lifecycle use where representable. Do not normalize text or serialize objects implicitly.

### 6. Key generation

Generate each pair from separate secure entropy requests using established implementations, returning public bytes and purpose-scoped private capabilities. Never convert between signing and agreement keys. Test pinned public keys from fixture inputs, independent entropy consumption, generation failure and cleanup if the second generation fails. Identity will compose these operations with persistent storage later.

### 7. Signatures

Sign exact caller-defined bytes through authorized private capabilities and verify 32-byte public keys and 64-byte signatures. Use ordinary Ed25519, not Ed25519ph; document backend validation behavior against the specified algorithm and investigate interoperability ambiguities before choosing extra wire rules.

Match the fixed Transport AUTH signature (CR-02); reject changed messages, signatures, domains, wrong keys and malformed lengths. Test denied key access and ensure no unrestricted remote signing endpoint is introduced.

### 8. Agreement

Support persistent and consuming ephemeral key capabilities while keeping their lifetimes distinct. Reject primitive failures and all-zero shared results with constant-time checking, even if the backend reports success. Compare the fixed envelope shared secret and test low-order inputs, wrong lengths, injected failures and cleanup (CR-08); never return the raw secret as an application encryption key.

### 9. Key derivation primitives

Preserve exact salt and info bytes and explicit output sizes, including Transport’s 32-byte keys and 16-byte nonce prefixes. Keep PRKs and key outputs protected; validate the algorithm’s output bound before allocating. Compare pinned extract/expand results and independent algorithm vectors, and test boundary lengths, changed context, errors and cleanup. Domain labels belong to the owning construction.

### 10. AEAD primitives

Use established XChaCha20-Poly1305 with 32-byte keys, 24-byte nonces, 16-byte tags and exact associated data. Define nonce assignment responsibility explicitly: owning constructions supply their prescribed fresh material or sequence and must not reuse a key/nonce pair. Do not create a competing generic wire format.

Bound input and output lengths before allocation; keep decryption output private until authentication succeeds and discard failed staging buffers. Test empty and nonempty inputs, wrong keys/nonces/AAD, missing or changed tags and limits against independent fixed values.

### 11. Integrity tags

Add the empty-plaintext AEAD construction over the exact Transport header/payload associated data, returning or verifying only the 16-byte tag. Compare upstream frame tags and test mutations and wrong directional material. Payloads remain visible; Transport owns nonce-prefix plus big-endian sequence construction, replay rejection and counter exhaustion.

### 12. Recipient credentials

Construct exactly ASCII("CR2SE-ENCRYPTION-KEY-V1") followed by the 32-byte X25519 public key, without a zero separator or length prefix. Sign through the authorized capability and verify SHA-256(0x01 || 0x01 || Ed25519 public key) against the claimed ID before accepting the binding.

Match the pinned binding and test altered ID, signing key, encryption key, signature and 63/65-byte signatures (CR-03). Credential validation does not assign trust or select between conflicting persistent keys; persistent credential/cache conflict policy belongs to Identity.

### 13. Envelope header

Encode and decode the exact 132-byte header: four suite bytes, recipient ID, persistent key, ephemeral key, nonce and unsigned big-endian sealed length. Require version/suite 1 and sealed length at least 16. Compare literal vector bytes and field offsets; test every header truncation, unknown selector and arithmetic boundary. Validate fixed fields before any size-dependent work.

### 14. Object boundaries

Build complete-object parsing and checked size calculations on the header codec. Require exactly 132 plus sealed length bytes, no trailing bytes, and plaintext length within configured policy. Validate a header before requesting its body; a complete-buffer API may expose this through a bounded caller adapter rather than implementing network I/O.

Test sealed lengths below 16, U64_MAX, host-size conversion, exact limits, one-over limits, truncated bodies and extra bytes (CR-07). Use synthetic lengths rather than enormous allocations; missing tags must never yield plaintext.

### 15. Envelope KDF

Build the 124-byte context from suite, recipient ID, recipient persistent key, ephemeral key and nonce in that order. Hash the specified salt prefix plus explicit zero separator and context, then extract and expand using the distinct key prefix plus zero separator and context.

Compare context, salt, PRK and derived key with CR-04’s fixed values through test-only observation. Mutate each context component and check the expected separation; erase intermediate material on every exit without exposing it in production diagnostics.

### 16. Envelope encryption

Compose credential validation, checked limits, fresh ephemeral key and nonce, agreement, exact KDF, header and AEAD into one bounded operation. Authenticate the full header and append the exact sealed bytes; provide no additional external AAD parameter. Check the complete pinned object (CR-04) and empty/nonempty recipient and self-encryption (CR-05).

Validate failures before unnecessary work and test entropy failure after ephemeral generation, agreement failure and subsequent construction failures. Every new object consumes fresh material; cleanup must already work on every error path, including before all temporaries exist.

### 17. Envelope decryption

Validate the complete object and compare recipient ID and persistent public key before agreement. Derive the exact key and authenticate the entire header before returning any plaintext. Test the fixed object, self/other-recipient round trips, wrong private keys, recipient mismatches, truncation and a one-bit mutation at every header/ciphertext/tag position (CR-04 through CR-07). Return no provisional plaintext and document the externally visible failure mapping.

### 18. Lifecycle and privacy

Extend lifecycle fault tests across every acquired secret in success, dependency failure and cancellation or caller abandonment where supported. For synchronous operations, document that cancellation means dropping an operation before/after the call rather than claiming unsupported mid-call interruption. Capture structured traces and errors to check that keys, PRKs, plaintext and raw payloads never appear.

Test two scripted fresh encryptions of identical plaintext and verify distinct consumed key/nonce material and successful decryption (CR-09). These are regression checks for cleanup implemented throughout earlier steps, not a delay in adding cleanup.

### 19. Transport adapter

Implement the existing Transport Cryptography trait with real primitives and opaque secret types, preserving consumed ephemeral ownership and DependencyError mappings. Keep cross-module wiring at the composition boundary. Test every port operation, 32-byte HELLO randomness, key/prefix sizes, signature rejection, all-zero agreement, tags and failure translation.

Use pinned Transport vectors to exercise transcript-derived material and protected frames with actual crypto and test-owned Identity credentials. Confirm derivation remains after peer AUTH verification and record the new upstream negative cases. Update Transport’s acceptance evidence only for assertions actually executed; real Identity persistence and the authenticated two-node server stay in roadmap steps 4–5.

### 20. Construction boundaries

Cover CR-10 with an actual pinned Messaging-envelope example rejected as a generic envelope and a test-only opaque store/retrieve path requiring no private key at the intermediary. Do not implement Messaging or paid Storage. Verify that successful generic decryption does not produce a sender-authentication claim and that Transport integrity does not imply confidentiality.

For CR-11, authenticate independent objects in swapped order to demonstrate that primitive authentication alone cannot establish sequence completeness. A test-owned manifest binding positions, lengths and hashes must reject reordered or missing objects; clearly label it an application scenario, not a new CR2SE manifest or chunk format.

### 21. Acceptance review

Maintain an acceptance matrix for every CR-01 through CR-11 assertion and applicable AT-C01 through AT-C04 rows, linking executable tests, fixture revisions and outcomes. Include capability misuse, limits, unavailable dependencies, immutable outputs and output discipline. Record why durability, remote commit uncertainty or serialized adapters are inapplicable to the supported in-process boundary rather than marking unexecuted rows passed.

Recheck pseudocode section 18 and cross-module responsibilities, including persistent installation/conflict handling deferred to Identity. Add missing focused tests and investigate discrepancies; local round trips alone are insufficient. Record only confirmed language-independent upstream defects in CR2SE bugs.

### 22. Completion and handoff

Update the module README, companion contract documents, root README, roadmap and affected CLI overview/detailed help together. Document build/run/test commands, dependencies, limits, secret lifecycle, verified vectors and the absence of sender authentication or forward secrecy in the generic envelope. Preserve the explicit unavailable server behavior until real Identity and composition are implemented.

Run formatting, Clippy with warnings denied, unit/integration/doc tests, build and overview/detailed help checks required by AGENTS.md. Measure production line coverage using docs/coverage.md and keep it above 80%, recording the percentage or an explicit measurement blocker. Mark steps complete only after evidence and documentation are current; list remaining Identity and real-node integration work without claiming node conformance.
