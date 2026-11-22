# Messaging module plan

Status: in progress; steps 1-18 are complete. Revised on 2026-11-22 after peer policy (milestone 12) was completed.

This is roadmap milestone 13 and planning folder 11, following peer policy
(milestone 12, [plan](../12_Peer_policy/PLAN.md)). Messaging is optional in
CR2SE's minimum profile but required by TalkSphere: the full client journey
needs it, and the hosted pages and feeds proposal
([plan](../13_Hosted_publications/PLAN.md)) reuses its placement rules.

## Steps

- [x] **1.** Reconcile Messaging requirements with the existing module contracts.
- [x] **2.** Define Messaging types and dependency ports.
- [x] **3.** Add deterministic Messaging test helpers.
- [x] **4.** Import the pinned Messaging definition and independent fixtures.
- [x] **5.** Implement exact Messaging price calculations.
- [x] **6.** Validate Messaging offering terms and advertised limits.
- [x] **7.** Validate operation-specific inputs and outputs.
- [x] **8.** Build and verify the exact message signing bytes.
- [x] **9.** Build and verify recovery request and receipt signatures.
- [x] **10.** Implement the encrypted Messaging payload format.
- [x] **11.** Publish and validate the Board identity encryption credential.
- [x] **12.** Persist bounded payload staging and placement records.
- [x] **13.** Implement placement states, expiration and per-placement serialization.
- [x] **14.** Implement durable participant preparation, commit, abort and status queries.
- [x] **15.** Recover Messaging participants before admitting new work.
- [x] **16.** Assess store metadata before receiving the payload body.
- [x] **17.** Commit validated stores as durable pending placements.
- [x] **18.** Implement recipient-only discovery with the exclusive cursor.
- [ ] **19.** Implement atomic recovery with a signed provider receipt.
- [ ] **20.** Return cached recovery output for an exact retry without a new charge.
- [ ] **21.** Implement sender-only status with the recorded receipt.
- [ ] **22.** Implement guarded renewal for pending and recovered placements.
- [ ] **23.** Quote early-release credits before removal.
- [ ] **24.** Settle the removal charge and release credits once in one recoverable decision.
- [ ] **25.** Retain removal replay results for the lifetime of the ledger.
- [ ] **26.** Implement authorized paid byte evidence for pending placements.
- [ ] **27.** Verify Messaging checks using private expectations and trusted timing.
- [ ] **28.** Reclaim expired, recovered and removed placement resources safely.
- [ ] **29.** Bound placements per sender and per recipient through a module limits port.
- [ ] **30.** Register Messaging handlers and publish usable offerings.
- [ ] **31.** Bind Messaging requester operations to Peer Protocol.
- [ ] **32.** Validate recovered messages and decrypt only after authentication.
- [ ] **33.** Exercise crash recovery at every coordination boundary.
- [ ] **34.** Verify races between recovery, removal, renewal and expiration.
- [ ] **35.** Exercise Messaging between authenticated node processes with replicas.
- [ ] **36.** Add a discoverable Messaging exercise and detailed help.
- [ ] **37.** Complete the Messaging acceptance matrix.
- [ ] **38.** Publish the Messaging completion and client journey handoff.

## Baseline, scope and review

Planned on 2026-11-22 against
[CR2SE bc53df4](https://github.com/beothorn/CR2SE/tree/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16).
Rechecked the same day: upstream main is now
[73edf2d](https://github.com/beothorn/CR2SE/commit/73edf2dc230bccb0f27f2ea6e06e938fff738862), which changes only the Peer
Protocol `resource_limit` text (proposal [#13](https://github.com/beothorn/CR2SE/issues/13), still open) and nothing in
Messaging. Recheck again before step 1 and record any newer change.

- [Messaging specification, including the definition and price formulas](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/modules/messaging/README.md).
- [Input contract](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/modules/messaging/inputs/README.md).
- [Output contract](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/modules/messaging/outputs/README.md).
- [Pseudocode](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/modules/messaging/pseudocode/README.md).
- [Messaging acceptance cases MS-01–MS-17](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/modules/messaging/acceptanceTests/README.md).
- [Composition rules for optional modules](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/ModuleContracts.md#required-and-optional-capabilities).

Existing local support to reuse rather than duplicate:

- Services already defines the narrow Messaging retransmission contract
  (`src/modules/services/replay.rs`) for recovery and removal replay IDs.
- Peer Protocol already projects the `payloadSize` pricing input for recover and
  renew, binds `releaseCredits` separately for remove and decodes explicit
  Messaging retransmissions (`src/modules/peer_protocol/{quote,requester,replay}.rs`).
- Cryptography already has an independently generated Messaging envelope
  fixture (`src/modules/cryptography/test_support/fixtures/messaging.md`) and
  the X25519, HKDF and XChaCha20-Poly1305 primitives.
- Storage is the structural model: provider participant, durable records,
  recovery, requester client and acceptance matrix under `src/modules/storage/`.
- Peer policy (milestone 12, [handoff](../../src/composition/policy/HANDOFF.md)) already provides, in node
  composition: per-identity request and failure rates for every stream a peer opens (so probing of placement
  IDs is already rate-limited without Messaging code), `Credit::release` for the early-release amount,
  `Owners::observe` for verified check outcomes, the live connection registry that lets a provider answer an
  outbound-only recipient, and the redundancy rules and report a sender can apply to its placements.

In scope: the complete version 1 provider and requester under
`src/modules/messaging/`, the Board `identityEncryption` field, the encrypted
payload format, durable placements and replay results, Services/Ledger
settlement of release credits, and process-level tests between TalkSphere nodes.
Out of scope: installing Messaging in `serve --node-root` and its `node-ctl`
commands (the [client journey](../14_Client_journey/PLAN.md), milestone 17),
hosted pages and feeds, Discovery, the Node API and interoperability with
another implementation.

Reuse: placement storage, states, retention, renewal, removal, checks and
replay results get clear internal boundaries inside `src/modules/messaging/`,
but are not extracted into a shared module yet. Hosted pages and feeds will be
the second user; the extraction happens in their plan, once the accepted
specification shows exactly what is shared.

Policy: the module never depends on node composition. It exposes small ports for the decisions CR2SE leaves
local (the early-release amount, per-sender and per-recipient placement limits) and the node composition fills
them from the peer policy contracts when the client journey installs Messaging. Module tests use fixed port
implementations.

Deployment fit: senders and recipients are typically phones that only connect outward. Every Messaging
operation is initiated by the sender or the recipient, so a provider never has to reach a phone; recipients
poll with `discover`, which is charged, so how often a client polls is client policy, not module behavior.

Messaging persistence is local and independent of the paid Storage service, as
upstream requires (MS-17). Payload size is bounded by finite configuration and
the logical value budget; the actual bound is recorded in step 1 rather than
assumed here.

## Execution rules

Follow [AGENTS.md](../../AGENTS.md) for each step: unit tests with failure and
boundary cases, integration tests at real boundaries, entry tracing, companion
contract Markdown, current README and help, and production coverage above 80%
measured with [the coverage procedure](../../docs/coverage.md). Keep protocol
code under `src/modules/messaging/`, split by responsibility. Commit each
verified step separately with a descriptive title and body. Check a step only
after its tests and docs pass.

Search open and closed upstream issues before reporting a confirmed protocol
defect, file new language-independent findings on GitHub and keep the local
record under [CR2SE bugs](../../CR2SE%20bugs/README.md). No new protocol defect is
asserted by this plan.

## Details

### 1. Reconcile Messaging requirements with the existing module contracts

Map every MS-01–MS-17 assertion and every specification rule to the owning
local module and to existing hooks in Services, Peer Protocol, Ledger, Board and
Cryptography. List what is missing, notably release-credit issuance inside one
recoverable decision and the Board encryption field. Record the payload bound
the finite profile can support. Recheck upstream main. Write
`src/modules/messaging/REQUIREMENTS.md`; no code changes.

Completed: [Requirements](../../src/modules/messaging/REQUIREMENTS.md). Payloads are bounded at 16384 bytes in the finite profile; four gaps were found, the main one being that release credits are not yet part of a Services decision.

### 2. Define Messaging types and dependency ports

Create `src/modules/messaging/` with typed IDs (message, placement, recovery,
removal), metadata, encodings, placement states, operation inputs and outputs,
and errors. Define ports for clock, entropy, durable placement storage, signing
and verification. Add the module README and companion contract documents. Test
fixed lengths, encoding codes and error mapping.

Completed: [module guide](../../src/modules/messaging/mod.md) with identifiers, errors, states, metadata, operation types and the clock, entropy and signer ports. The durable placement store port arrives with step 12, where its shape is known.

### 3. Add deterministic Messaging test helpers

Add test-only clock, entropy, identities and in-memory placement storage with
injectable failures, mirroring Storage's `test_support`. They are never exposed
to production code. Test that the helpers are deterministic and that injected
failures surface as typed errors.

Completed: [test doubles](../../src/modules/messaging/test_support.md) for the clock, entropy and identities with real Ed25519 keys, each with injected failures. The in-memory placement store double is built with the store port in step 12, where that port is defined.

### 4. Import the pinned Messaging definition and independent fixtures

Add the version 1 definition from upstream at the pinned revision and parse it
through Services schema validation. Add independently generated fixtures for
signing bytes, recovery request and receipt bytes and the MS-01 example, with a
generator script and README naming its source revision. Tests consume literal
bytes, never Rust-generated expectations.

Completed: [standard catalogue](../../src/modules/messaging/standard.md) over the pinned definition and [fixtures](../../src/modules/messaging/fixtures/README.md) generated with PyNaCl; tests show Rust derives the same identities and verifies, and reproduces byte for byte, the Python signatures for MS-01, MS-06 and MS-07.

### 5. Implement exact Messaging price calculations

Implement store, recover and renew prices with exact rational terms added before
one ceiling, the minimum price and the fixed discover, status, remove and check
prices. Use overflow-safe arithmetic and reject results above `uint64`. Test the
MS-02 price 17, the MS-03 half-factor case (1, not 2), zero denominators and
maxima.

Completed: [pricing](../../src/modules/messaging/pricing.md). The arithmetic is the same exact algorithm Storage uses, kept as Messaging's own file so the two modules stay independent; the acceptance values 17, 1, 2 and 6 and the overflow, zero and floor cases are tested.

### 6. Validate Messaging offering terms and advertised limits

Parse a complete offering: every pricing and `info` field, the
`cr2se.identity` precondition, `pricing` instead of `price`, issuer equal to the
publisher, and the duration ordering rules. Test each missing, duplicate,
inconsistent and overflowing field.

Completed: [terms](../../src/modules/messaging/terms.md) with tests for every member, rate, consistency rule, unsupported model and overflowing maximum. The finite-profile payload cap (16384 bytes) is enforced where the provider is configured, not in generic term validation.

### 7. Validate operation-specific inputs and outputs

Implement the operation discriminant: required fields per operation, rejection
of known fields from another operation, paired cursor fields, limit bounds,
fixed lengths and non-empty payloads. Validate outputs the same way for the
requester. Test each operation with missing, extra and malformed fields.

Completed: [codec](../../src/modules/messaging/codec.md) for requests, responses and the check, with round-trip and exhaustive negative tests (every required member, wrong types, wrong lengths, foreign, unknown and duplicate members, size mismatch, cursor and limit rules). `StoreOutput` gained the specified `messageId`.

### 8. Build and verify the exact message signing bytes

Build `CR2SE-MESSAGE-V1` signing bytes and verify sender key derivation,
authenticated caller, size, hash and Ed25519 signature in the order upstream
specifies. Test MS-01: the exact byte layout and that mutating any metadata
field or payload byte fails verification.

Completed: [signing](../../src/modules/messaging/signing.md); MS-01 passes against the independent Python vector (identical bytes and signature) and every single-field mutation is rejected.

### 9. Build and verify recovery request and receipt signatures

Build the recovery request bytes and the provider receipt bytes, sign them and
verify both. Test MS-06 and the receipt part of MS-07: each wrong provider,
message, hash, recovery ID, recipient key or signature fails.

Completed: [receipt](../../src/modules/messaging/receipt.md); MS-06 and MS-07 match the independent vector byte for byte and every single-value mutation of a request or receipt is rejected.

### 10. Implement the encrypted Messaging payload format

Implement the `cr2se.identity-encrypted.v1` payload: metadata bytes, ephemeral
X25519, HKDF salt and info, XChaCha20-Poly1305 and the `0x01` layout. Decryption
yields no plaintext on failure. Test MS-14 against the existing independent
fixture, plus wrong key, mutated metadata, tag tamper, version change and
truncation. Confirm the generic envelope is never substituted.

Completed: [encryption](../../src/modules/messaging/encryption.md); the output matches the independent PyNaCl payload byte for byte and every failure case yields no plaintext.

### 11. Publish and validate the Board identity encryption credential

Add the optional top-level `identityEncryption` Board field with strict Base32
decoding, publisher ID derivation, binding signature and X25519 validation. An
invalid field makes only the key unusable. Test MS-15, including unrelated
offerings staying usable and opaque messages working without a credential.

Completed: [credential](../../src/modules/messaging/credential.md): strict Base32, ID derivation, binding and contributory-key checks, and publishing the member. The node publishes its own member in the client journey (step 4 there).

### 12. Persist bounded payload staging and placement records

Persist incomplete uploads in bounded staging and accepted placements with all
fields upstream lists, atomically and owner-only. Placement persistence is the module's own local store, not
paid Storage and not the node's replica book. Reject placement ID reuse and
never replace a record. Test reopen, corrupt records, write failure and the
staging bound.

Completed: [placement](../../src/modules/messaging/placement.md) and [store](../../src/modules/messaging/store.md), volatile and native, with identical contract tests. The store is a concrete type with a volatile backing and injectable write failure instead of a trait. Incomplete uploads are held in memory under a bounded reservation (step 16) rather than persisted, because they create no obligation.

### 13. Implement placement states, expiration and per-placement serialization

Implement receiving, pending, recovered, removed and expired transitions against
the provider clock, with one serialized writer per placement. An operation
accepted while active completes under its terms. Test every allowed and refused
transition and the expiry boundary.

Completed: [lifecycle](../../src/modules/messaging/lifecycle.md); serialization is the store's atomic update, proven by threaded race tests.

### 14. Implement durable participant preparation, commit, abort and status queries

Implement the Services participant contract for Messaging effects, like Storage
steps 13–14: prepare durably, commit and abort idempotently, and answer status
queries. Test repeated commit and abort, unknown decisions and write loss.

Completed: [effects](../../src/modules/messaging/effect.md), [journal](../../src/modules/messaging/journal.md) and [participant](../../src/modules/messaging/participant.md). The participant works on Messaging's own typed effects; the Services `Provider` adapter (step 30) maps them onto Services blobs and decision-time finalization. Placement locks keep two operations from preparing on one placement.

### 15. Recover Messaging participants before admitting new work

On open, resolve prepared participants from Services decisions before new work
is admitted. Test recovery at each prepared state and that no new store is
accepted until recovery completes.

Completed: [recovery](../../src/modules/messaging/recovery.md), tested with native stores reopened after a simulated crash at the prepared state and in the apply-before-journal window.

### 16. Assess store metadata before receiving the payload body

Validate offering limits, retention, price, identity and signed metadata before
the payload body is accepted, and reserve bounded capacity. Test MS-03 cases
that can be refused before the body, with no charge and no reservation left
behind.

Completed: [staging](../../src/modules/messaging/staging.md) and [provider](../../src/modules/messaging/provider.md). Signature verification needs the payload, so it runs when the body has arrived; everything else is refused beforehand.

### 17. Commit validated stores as durable pending placements

Stream the payload into staging, verify size, hash and signature, then commit
the placement and return `placementId`, `acceptedAt`, `expiresAt` and `price`.
Test MS-02: durable before success, restart, and a second store of the same
message creating a separate placement.

Completed: `Provider::store` (assess, prepare, commit) with MS-02 and MS-03 tests in `provider_tests.rs`; streaming from the network is wired in step 30.

### 18. Implement recipient-only discovery with the exclusive cursor

Return the authenticated recipient's pending placements ordered by
`(acceptedAt, placementId)`, bounded by `limit`, with `more` and the exclusive
cursor. An empty page is a charged success. Test MS-05 and the discovery part
of MS-04.

Completed: `Provider::discover` (see [provider](../../src/modules/messaging/provider.md)); the store's `pending_for` now also hides expired placements.

### 19. Implement atomic recovery with a signed provider receipt

Verify the recovery request, then atomically move the placement to recovered,
record the receipt and cache the complete output. Test MS-07: original metadata
and payload returned, receipt verifies, discovery excludes the placement and the
output has no invented price field.

### 20. Return cached recovery output for an exact retry without a new charge

Use the Services replay contract so an exact retry by the same recipient
returns the byte-identical cached output and original price, even after restart
or a price change. A different recovery ID fails without charge. Test MS-08,
including the cache surviving a renewal of a recovered placement.

### 21. Implement sender-only status with the recorded receipt

Return pending or recovered state with the exact recorded receipt fields and no
payload, only to the sender and only before expiry. Test MS-09 and the status
part of MS-04.

### 22. Implement guarded renewal for pending and recovered placements

Renew only with the exact expected expiration, within the current duration and
total limits, without changing state. At most one concurrent renewal per
expected value succeeds. Test MS-10.

### 23. Quote early-release credits before removal

Compute one exact `releaseCredits` through the module's release port (filled by `Credit::release` in the
node), bounded by the minimum requested and the cumulative store and renewal payments. Bind it in the quote
before any change. Test the quote rules of MS-11 and MS-12, including that
discover, recover, status, remove and check prices never enlarge the ceiling.

### 24. Settle the removal charge and release credits once in one recoverable decision

Apply the remove debit and the release-credit issuance as two separate Ledger
records in one Services decision, with no underflow or overflow. Add the
missing Services or Ledger support found in step 1 in its owning module, with
its own tests and companion documentation. Test MS-11 and the refusal cases of
MS-12.

### 25. Retain removal replay results for the lifetime of the ledger

Persist removal results so an exact retry after a lost reply or restart returns
the identical output without applying either ledger change again, beyond the
placement's expiry. Test the replay part of MS-12.

### 26. Implement authorized paid byte evidence for pending placements

Serve one payload byte for a sender's pending placement under the common
Services check rules. Refuse invalid offsets, inactive placements and wrong
callers without charge. Test the provider side of MS-13.

### 27. Verify Messaging checks using private expectations and trusted timing

Derive pass, fail or inconclusive on the requester from the private expected
byte and `checkResponseSeconds`, posting no charge without valid evidence. Reuse
Storage's sampling and timing approach. Test the requester side of MS-13.

### 28. Reclaim expired, recovered and removed placement resources safely

Release payload bytes and staging when obligations end while keeping receipts,
recovery caches until expiry and removal tombstones as required. Test that
reclaim never removes data still owed to a retry or status query.

### 29. Bound placements per sender and per recipient through a module limits port

Refuse a store before any body or charge with `resource_limit` when the sender or the recipient already holds
the most pending placements the limits port allows, so unsolicited messaging stays bounded at each provider.
Request and failure rates are already applied node-wide by peer policy; this step only keeps unauthorized and
unknown placements indistinguishable in replies. Test both limits at their boundary, refusal without charge,
and the indistinguishability required by MS-04.

### 30. Register Messaging handlers and publish usable offerings

Register the provider handler with Services and publish the offering only when
the handler and its durable owners are ready. Test registration, refusal when
dependencies are missing and that nothing unfinished is advertised.

### 31. Bind Messaging requester operations to Peer Protocol

Add a requester client for store, discover, recover, status, renew, remove and
check using the existing quote, consent and retransmission machinery, with exact
spending bounds. Test each operation over real Peer Protocol endpoints with
volatile adapters.

### 32. Validate recovered messages and decrypt only after authentication

On the requester, verify sender key derivation, hash and signature, verify
recovery and status receipts, deduplicate by `(senderId, messageId)` and decrypt
only afterwards. Test MS-17 deduplication and each validation failure.

### 33. Exercise crash recovery at every coordination boundary

Reopen independent Services, Ledger and Messaging files at each boundary for
store, recover and remove, including after lost durability replies. Test that
balances never underflow, receipts never exist without matching accounting and
release credits never apply twice (SV-09 with Messaging effects).

### 34. Verify races between recovery, removal, renewal and expiration

Run concurrent recover against remove, recover near expiry, renew against
remove and status against transitions. Test MS-16: exactly one transition
succeeds and each reply reports a state that existed.

### 35. Exercise Messaging between authenticated node processes with replicas

Run sender, recipient and two providers as separate processes over
authenticated TCP. Store one encrypted message at both providers, discover,
recover, check status, then remove the other copy for release credits. This
uses the test harness, not `node-ctl`, which the client journey adds.

### 36. Add a discoverable Messaging exercise and detailed help

Add a runnable example under `examples/messaging/` and document it in the module
README and a `docs/messaging/EXERCISE.md` guide. Update any help topic that
names supported capabilities. Test that the example runs.

### 37. Complete the Messaging acceptance matrix

Write `src/modules/messaging/ACCEPTANCE.md` mapping MS-01–MS-17 to tests,
fixtures and remaining limits. Run the full check list and record coverage.

### 38. Publish the Messaging completion and client journey handoff

Write `src/modules/messaging/HANDOFF.md` with contracts the node composition
needs: the release and limits ports to fill from peer policy, check outcomes to pass to `Owners::observe`, and
how a sender's placements map onto the redundancy report. Update this plan, the roadmap and the root README,
and record known limits.
