# Identity module plan

Status: steps 1–6 are implemented, tested and documented; remaining steps are pending.

Identity is the next required module, roadmap step 4 and module folder 3.
It uses the completed Cryptography implementation and supplies protected local
identity authority to Transport and future service modules. This plan does not
start the server or implement later roadmap modules.

## Steps

- [x] **1.** Define Identity’s public contracts and dependency boundaries.
- [x] **2.** Add deterministic Identity helpers and pinned upstream fixtures.
- [x] **3.** Validate public signing keys against the updated CR2SE profile.
- [x] **4.** Align signature verification with the updated CR2SE acceptance rules.
- [x] **5.** Derive binary IDs and validate claimed public identities.
- [x] **6.** Format binary IDs as canonical Base32 text.
- [ ] **7.** Parse and normalize textual IDs without accepting ambiguous spellings.
- [ ] **8.** Format identity and contact QR payloads.
- [ ] **9.** Parse contact payloads with strict endpoint validation.
- [ ] **10.** Manage configured contact snapshots without changing their expected identities.
- [ ] **11.** Restrict identity operations to purpose-scoped local capabilities.
- [ ] **12.** Provide protected key-store access through explicit Cryptography contracts.
- [ ] **13.** Encode complete local credential records within fixed bounds.
- [ ] **14.** Implement protected credential reads for the supported platform.
- [ ] **15.** Install both identity key pairs atomically and durably.
- [ ] **16.** Create identities from independent keys and a valid binding.
- [ ] **17.** Load existing identities without replacing missing or inconsistent keys.
- [ ] **18.** Persist configured contacts with atomic snapshot updates.
- [ ] **19.** Reject unsupported identity and encryption-key changes.
- [ ] **20.** Sign exact protocol bytes through authorized identity handles.
- [ ] **21.** Verify identity proofs without confusing key relations with possession.
- [ ] **22.** Decrypt generic envelopes through authorized persistent-key handles.
- [ ] **23.** Back up both identity key pairs through a privileged local operation.
- [ ] **24.** Restore protected backups without silently changing identity authority.
- [ ] **25.** Verify recovery boundaries with a separate application catalog.
- [ ] **26.** Verify cleanup and restart behavior across identity failures.
- [ ] **27.** Verify private data stays out of diagnostics and public results.
- [ ] **28.** Adapt real Identity operations to Transport’s dependency port.
- [ ] **29.** Audit Identity against every applicable upstream acceptance case.
- [ ] **30.** Document Identity completion and the server integration handoff.

## Reference baseline and scope

Reviewed against CR2SE revision
[`d40b6278f24430e814d7fa82094f57261bfcac9f`](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f).

- [Identity specification](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/identity/README.md).
- [Input contract](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/identity/inputs/README.md).
- [Output contract](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/identity/outputs/README.md).
- [Identity pseudocode and minimum cases](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/identity/pseudocode/README.md).
- [ID-01–ID-10 acceptance suite](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/identity/acceptanceTests/README.md).
- [Module contracts and ownership](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/ModuleContracts.md).
- [Common acceptance rules](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/AcceptanceTests.md).
- [Transport fixed vectors](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/transport/pseudocode/TestVectors.md).
- [Updated Cryptography profile](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/cryptography/README.md).
- [Cryptography negative vectors and cleanup cases](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/cryptography/acceptanceTests/README.md).

The upstream revision resolves the reported weak-point ambiguity and cleanup
placement issues ([#6](https://github.com/beothorn/CR2SE/issues/6),
[#7](https://github.com/beothorn/CR2SE/issues/7)). TalkSphere's completed
Cryptography evidence remains pinned to its earlier revision until steps 3–4
and the lifecycle review verify the new requirements. No new confirmed protocol
defect was found during this planning review; platform storage formats and
protection mechanisms are explicitly implementation choices.

Keep code under `src/modules/identity/`, separated by responsibility into ID/text
codecs, contacts, credential lifecycle, authority and storage adapters as needed.
Keep cross-module wiring in `src/composition/`; extend Cryptography through public
contracts only. Add folders when they contain work, with companion Markdown for
public contracts and module documentation explaining callers and dependencies.

Every implementation step includes unit tests, relevant invalid/failure cases,
safe entry tracing and current contract documentation as required by
[AGENTS.md](../../AGENTS.md). Use integration tests for persistence, process
restart and composition; final audits consolidate evidence rather than postpone
testing. Checklist entries stay unchecked until the implementation, tests and
affected documentation are complete.

The initial deployment is direct in-process Rust with a protected Linux/Unix
filesystem adapter, not a process-separated service or universal hardware key
store. One owner-only record keeps the complete key association together; its
protection relies on OS access controls, not an invented CR2SE encryption format.
The plan includes privileged local backup/restore and durable contacts, while
application catalogs, trust, balances, service obligations, Discovery runtime,
QR image rendering, Node API and full server startup remain outside this module.

Dependencies follow checklist order: contact handling is introduced as an explicit
core/port before the later durable adapter, and creation follows atomic storage.
Recheck upstream before implementation and record any newly adopted revision.

## Step details

### 1. Public contracts

Define exact binary IDs, validated public identities, complete public credentials, contacts, immutable limits, opaque identity handles and typed failures under `src/modules/identity/`. Keep malformed input, mismatched identity, invalid binding, unavailable storage, denied authority, conflict and uncertain local installation distinguishable without revealing protected resource existence.

Declare Cryptography, protected key-store and configured-contact storage ports, with local caller authority and contract version checked before effects. Bound text, contacts, signing bytes, envelopes and local records. Add scaffold tests for limits and unsupported/unavailable operations; no successful placeholder creation. Describe direct in-process support without promising IPC or platform coverage.

Completed: Defined typed identity/contact snapshots, immutable budgets, fixed failures and explicit crypto/store ports; unavailable lifecycle defaults return no credentials. All required checks and 230 tests pass; production coverage 95.33% (3654/3833).

### 2. Dependency helpers and fixtures

Provide test-only scripted Cryptography, entropy, key-store and contact-store substitutes with ordered calls, restart simulation and injected failures. Record acquisitions, cleanup and durable state without exposing secrets through production APIs.

Pin both Transport identities/text forms, AUTH bytes and the generic envelope/binding vector to the reviewed revision. Add the new Ed25519 negative vectors with provenance and literal expected values. Test helper scripts and rejection of exhausted fixtures; never let fixture entropy reach production composition.

Completed: Pinned current Transport/envelope/negative-signature fixtures and added strict ordered crypto scripts, entropy, restart/failure storage and immutable contact substitutes. All required checks and 233 tests pass; production coverage remains 95.33% (3654/3833), excluding test support.

### 3. Public verification-key validation

Add the missing Cryptography public-key validation contract needed by Identity; keep curve operations in Cryptography and use established libraries. Check exact width, canonical compressed encoding, on-curve decoding, re-encoding and small-order rejection before returning a validated verification key. Hashing a key into an ID remains a separate operation.

Exercise upstream invalid A values even when their claimed IDs hash correctly, including noncanonical y and the forbidden x=0 sign bit. Do not impose prime-subgroup-only rejection: the new profile explicitly permits mixed-order points subject to its equation. Test valid fixture keys, malformed lengths and dependency failures; update affected companion contracts.

Completed: Added canonical non-small-order VerificationKey parsing with explicit decode/re-encode checks and mixed-order acceptance. Both fixed keys and invalid width/point/encoding cases pass. All 234 tests and required checks pass; production coverage 95.35% (3669/3848).

### 4. Cryptography dependency alignment

Review the current `verify_strict` path against every rule in the new profile rather than assuming its name establishes equivalence. Enforce canonical R, S < L, small-order rejection and the uncofactored exact equation using established primitives, reusing step 3. Preserve ordinary Ed25519 and exact honest AUTH/binding vectors.

Execute the new CR-02 negative rows and equivalent individual decisions for a collection containing valid and invalid signatures; do not advertise an unsupported batch API. Record regression evidence and the adopted revision in Cryptography's acceptance report, resolving its historical local-policy caveat only after validation. This is a focused dependency correction required by Identity, not reopening unrelated Cryptography work.

Completed: Aligned canonical A/R validation with backend scalar bounds and the uncofactored equation; all new negative rows, mixed-order acceptance/rejection and existing AUTH/binding vectors pass. All 236 tests and required checks pass; production coverage 95.35% (3674/3853).

### 5. Binary identities

Implement version/algorithm checks and SHA-256 over exactly 0x01 || 0x01 || the 32-byte signing key through Cryptography. Use binary IDs for equality and map the existing Transport ID at the composition boundary instead of reading another module's internals.

Public identity validation compares the claimed ID and applies step 3 key validation; derivation alone hashes bytes and establishes no possession. Cover ID-01, ID-06 and pseudocode length/version/algorithm cases, both fixed IDs, mismatches and no authenticated-connection or trust side effects.

Completed: Implemented exact version/algorithm ID derivation and claimed-key validation through scripted and real public Cryptography ports. Both fixed IDs, malformed fields, weak keys and dependency errors pass. All 239 tests and required checks pass; production coverage 95.43% (3734/3913).

### 6. Text formatting

Emit the exact lowercase `cr2se:` prefix and 52 uppercase RFC 4648 Base32 symbols with no padding for 32 bytes. Document the 58-byte output and zero unused final bits. Test both upstream textual vectors, all-zero/all-one IDs and binary boundaries against independent expectations, not only local round trips.

Completed: Canonical text formatting matches both fixed vectors and all single-bit/extreme IDs with exact prefix, alphabet, length and unused bits. All 240 tests and required checks pass; production coverage 95.45% (3755/3934).

### 7. Text parsing

Require the exact prefix and byte length; accept ASCII case variations only in Base32. Reject whitespace, padding, Unicode lookalikes, invalid characters and nonzero unused final bits without trimming or Unicode normalization. Normalize by parsing then canonical formatting.

Cover every ID-02/03 mutation, all unused-bit alternatives for a final symbol and binary equality across letter casing. Keep bounded parsing independent of entropy, storage and networking.

### 8. QR payload formatting

Return textual payloads rather than generating QR images. An ID alone uses canonical text; a contact adds one IP literal and a decimal port in 1..65535. Emit address then port as a documented local ordering, percent-encoding non-unreserved UTF-8 bytes so IPv6 colons are escaped; omit IPv6 brackets and zones.

Test exact IPv4/IPv6 and port-boundary outputs, ID-only output and no private information. Contact hints never alter the ID; no general URL library may silently normalize forbidden input.

### 9. Contact parsing

Accept ID alone or exactly one address and one port in either order. Decode percent escapes once; validate an IPv4/IPv6 literal and canonical decimal port. Reject missing/duplicate/unknown parameters, fragments, DNS names, brackets, zones, malformed escapes, signs, leading zeros, double escaping and invalid UTF-8. Apply a finite input bound before parsing.

Cover ID-07/08 and exact expected-ID preservation. A parsed contact is an immutable hint and performs no socket, DNS, Discovery or trust operation. Follow the specification's encoding requirements without inventing additional peer wire formats.

### 10. Configured contact ownership

Implement authorized bounded contact lookup and in-memory updates through an explicit contact-store port, independently of optional Discovery and paid Storage. Key contacts by binary ID; snapshot results must remain unchanged after later edits. Address changes must not replace an expected ID or grant trust.

Test duplicate/conflicting input, unauthorized mutation, capacity limits and immutable snapshots. This step defines the contact core and dependency port; durable production storage is a separate later step and is not advertised here. ID-only contacts remain valid but cannot locate an unknown network without a bootstrap hint.

### 11. Authority and capability lifetime

Issue opaque capabilities only from trusted local composition for public credential access, signing, decryption and privileged backup/import. Separate administrative authority from operation authority; bind each capability to its identity and purpose, with documented lifetime and resource limits. Peer payload fields cannot manufacture authority.

Use compile-fail and runtime tests for wrong-purpose/wrong-identity access, denied calls, expired or released handles where supported and checks before crypto/storage effects. Cover ID-09 and AT-C01/C03 without introducing Node API tokens, remote administrative endpoints or hidden network calls.

### 12. Key-store and Cryptography bridge

Extend only the public Cryptography contracts needed to derive public keys from authorized protected pairs and transfer generated/imported material directly into a guarded platform store. Do not reach into private Secret fields, add general raw-key getters or weaken purpose separation. A privileged backup transfer must remain distinct from ordinary module results.

Document ownership for generated, opened and serialized keys, zeroizing staging and failure cleanup. Test known seeds/public values, invalid key representations, denied access, partial dependency failure and signing/encryption purpose separation. Keep portable protocol bytes independent of the selected local storage representation.

### 13. Local credential record codec

Define a versioned implementation-specific record containing both protected key representations and their public association, including enough cached metadata to detect mismatches. This is not a CR2SE wire or portable backup format. Use bounded zeroizing staging buffers and exact checked lengths; reject unknown versions, truncation and trailing bytes.

Test deterministic records and malformed fields without recording production secrets. The codec does not publish or install keys; record validation and atomic storage remain separate. Document how the record version can be rejected safely instead of silently migrating or regenerating an identity.

### 14. Protected filesystem adapter

Provide an initial Linux/Unix local key-store adapter using an owner-only directory and owner-only regular files, with no-follow, directory-anchored access and explicit owner/mode validation. Acquire an exclusive writer lock for the active identity store; unsupported platforms return an explicit error until a protected adapter exists. Document the threat boundary: OS access protection is not file encryption or protection from the same account/root.

Test permissions, symlinks, path traversal, oversized files, missing/corrupt records, lock contention and unavailable storage using temporary directories. Never accept peer-supplied paths. Check protection before loading secret bytes and erase temporary buffers on failure.

### 15. Atomic installation and restart

Stage one complete credential record under the protected directory, sync required contents, install without overwriting an existing identity, and sync directory metadata before reporting durable success. Define status/reconciliation for a lost acknowledgement or a failure after the commit point: never claim rollback or silently generate replacement keys when installation may have committed.

Test each write/sync/install cut, fresh-process restart, no partial usable pair, no-overwrite races, and cleanup of abandoned protected staging. Document platform guarantees and the distinction between uncommitted failure and uncertain completion.

### 16. Identity creation

Compose authorized generation of independent Ed25519 and persistent X25519 pairs, ID derivation, exact key binding and atomic storage. Acquire cleanup ownership immediately; return the public credential and scoped handles only after both pairs and their association are durable. Grant no trust, credits or inherited service state.

Cover ID-04/05 with real Cryptography and scripted entropy/storage failures after each acquisition, including binding failure and second-pair failure. Verify no key conversion, no registration/network call, correct binding bytes, no incomplete public success and no automatic retry that creates another identity.

### 17. Validated loading

Open the configured pair through the protected store, derive both public keys and ID, validate/recompute the binding and compare all cached public fields. Reject missing keys, wrong private/public association, corrupted metadata, unavailable storage and invalid bindings. Do not repair an existing identity by silently creating new keys.

Test restart returns the same exact credential, both private authorities remain usable, wrong/partial records fail and all opened staging is cleaned. Exercise the protected platform adapter as well as fault substitutes; tests cover ID-04/05 and the pseudocode cached-field requirements.

### 18. Durable configured contacts

Connect the contact core to the protected filesystem adapter using a separate bounded, versioned local record. Publish complete snapshots atomically and acknowledge success only after durability; keep private key records and contact ownership separate. This is implementation-specific persistence, not a new contact wire format.

Test reload in a fresh process, write/sync failures, corrupted records, concurrent updates, denied access and unchanged previously returned snapshots. Preserve each expected binary ID across endpoint updates and reconcile uncertain writes without hidden Discovery or paid Storage calls.

### 19. Identity selection and key conflicts

Validate configuration selecting exactly one active local identity. Classify unchanged keys, a new signing identity and the same ID with a different persistent encryption key; reject transparent replacement/rotation and never select among conflicting valid bindings silently. Apply the same conflict policy to retained recipient credentials.

Test zero/one/multiple configured identities, validly signed competing encryption keys, independent nodes loaded with identical pairs and distinct signing keys yielding distinct IDs. Document one active writer per identity for the PoC and that copying keys does not replicate Ledger, Board or service state; this module cannot enforce coordination across unknown machines.

### 20. Scoped signing

Delegate bounded, caller-defined canonical bytes to Cryptography using the purpose-scoped signing authority. The consuming protocol owns domain separation, roles and freshness; Identity must not introduce serialization, a generic challenge exchange or a reusable signature over only the ID.

Test fixed signatures, exact-byte mutation, wrong authority/identity, oversize inputs, inaccessible keys and dependency failures. Return only complete signatures and preserve distinct secret-free failures; grant no trust or authenticated connection merely because a local signature was produced.

### 21. Proof validation

Combine validated claimed identities with exact-message signature verification. Require the caller's protocol-defined bytes and reject wrong IDs, keys, messages, domains and roles; retain the separation between public-key relation checks and fresh possession proofs.

Cover ID-06 and malformed/weak-key negatives under the adopted profile. Test dependency failures and confirm no connection-authenticated event or replay state is invented by the helper; Transport remains responsible for its transcript and freshness state machine.

### 22. Local decryption

Expose explicit generic-envelope decryption with expected local ID/key and bounded requests, delegating to the existing Cryptography construction. Grant authority only to the intended local identity; select formats explicitly and return unsupported for unimplemented service-specific formats instead of guessing.

Test the fixed envelope and generated round trips, wrong identity/key/purpose, denied authority, malformed/tampered objects, missing tag, limits and unavailable storage. No provisional plaintext or secret logging is allowed; valid decryption does not establish sender authentication or trust.

### 23. Protected backup

Provide an explicitly authorized platform backup operation that snapshots both pairs and their association together into a protected local destination using the documented local format. Do not export private keys through ordinary public credentials, logs, stdout or peer protocols. Require the same access protection and atomic publication as the key store.

Test unauthorized backup denial, both pairs present, destination conflicts, write failure, cleanup and a stable snapshot while public state changes. Document that copying a permission-protected file to an unprotected destination is not encrypted backup; the caller must secure the independent backup location.

### 24. Validated restore

Import through privileged local administration into a new protected store, validating both pairs, binding, metadata and format before atomic installation. Reject incomplete/corrupt backups, cached mismatches and conflicting existing identities; do not invent encryption-key rotation or overwrite a live identity implicitly.

Test restore in a fresh process/store yields the same ID, signature capability and decryption of an object encrypted before backup. Inject failures before and after installation and reconcile uncertain completion without regenerating keys. Backup format portability is local and documented, not an asserted CR2SE export standard.

### 25. Recovery scenarios

Cover ID-10 with a test-owned independent catalog containing provider/contact/lease/hash/format metadata and both restored key pairs. Confirm the catalog remains the application's responsibility, not Identity's database or a newly implemented Storage enumeration service.

Test keys without a catalog make no recovery-location claim; signing-only recovery cannot decrypt old ciphertext; changing signing keys creates a new ID without inherited obligations. Restore keys and the catalog from separate test locations and distinguish successful cryptographic recovery from the later paid multi-node recovery milestone.

### 26. Lifecycle and fault audit

Consolidate acquisition-to-release observations across creation, loading, signing, decryption, backup and restore. Extend the new CR-09 fault cases that apply to Identity, including generation, public derivation, binding and atomic storage; document infallible backend boundaries rather than inventing unsupported fault injection.

Exercise restart at durable cuts, denied/abandoned calls and cancellation before work or after a commit point. Synchronous calls do not promise mid-call interruption; acknowledged state must survive, uncertain outcomes must be reconciled, and partial keys must not become usable. Tests throughout earlier steps remain required; this step closes coverage gaps.

### 27. Output privacy

Capture trace events, formatted errors, public credentials, textual IDs, QR payloads and denied-operation results during real workflows and failure cases. Check fixture secrets, backup bytes and decrypted payloads never appear outside authorized results; public keys and IDs alone do not grant private authority.

Test bounded diagnostics for hostile text and filesystem failures without echoing secrets or trusted claims from unvalidated input. Exercise immutable snapshots and released-handle behavior for AT-C03/C04; do not add production diagnostic byte extraction for tests.

### 28. Transport composition adapter

Implement the existing Identity port under `src/composition/`, mapping validated public identity, binary derivation and the Transport-created AuthenticationMessage to scoped authority. Keep raw key material inside protected storage/Cryptography contracts and preserve typed dependency failures.

Use real persisted/reloaded Identity and real Cryptography to exercise both handshake roles, pinned AUTH vectors, invalid proofs, denied key access and contact expected-ID mismatch before application dispatch. Confirm no new session keys or authenticated state are published before verification. This adapter does not enable `serve`; the full socket/node demonstration remains roadmap step 5.

### 29. Acceptance review

Create an evidence matrix for ID-01 through ID-10, the additional minimum cases in pseudocode section 15, applicable AT-C01–AT-C04 and the adopted Cryptography dependency regressions. Map every assertion to an executed test, fixture revision, platform and outcome; keep unsupported process-separated deployment and full-node recovery distinct.

Review the public contracts and failure paths for missing assertions; use real restart evidence for durability and fixed vectors rather than only round trips. Search upstream before filing any confirmed protocol issue, retain a local report with the issue URL and record unresolved blockers honestly. No module completion claim may rest on test-only storage.

### 30. Completion and handoff

Update the module README, public-contract companions, root README, roadmap, overview/detailed help and affected CLI tests with actual supported capabilities. Explain build/test/library usage, configured protection, limits, backup/restore, single-writer policy and remaining server composition. Do not advertise administrative commands unless implemented and tested; runnable library examples can exercise the module before CLI integration.

Run formatting, Clippy with warnings denied, locked unit/integration/doc tests, build and overview/detailed help checks from AGENTS.md. Measure production line coverage above 80% using docs/coverage.md and publish the measured result. Only then check off completed work and hand off to roadmap step 5; minimum-node conformance, service recovery and optional modules remain later work.
