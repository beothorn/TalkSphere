# Ledger module plan

Status: execution in progress; requirements review complete.

This is roadmap milestone 7 and planning folder 6, following completed Board.
Ledger owns local trust, issuer-separated balances, finite budgets, reservations,
settlement and durable transfer/claim replay records.

## Steps

- [x] **1.** Reconcile Ledger requirements and acceptance cases with upstream.
- [x] **2.** Define Ledger authority, operation identities and resource limits.
- [x] **3.** Add deterministic Ledger fixtures and failure helpers.
- [x] **4.** Implement exact issuer-separated accounting views.
- [x] **5.** Define atomic persistence and recoverable transaction outcomes.
- [x] **6.** Implement protected local Ledger persistence.
- [x] **7.** Implement authorized trust and finite credit administration.
- [x] **8.** Validate settlement roles and payment policies.
- [x] **9.** Reserve provider-issued spending without double use.
- [x] **10.** Reserve requester-issued credits and recipient capacity.
- [x] **11.** Reserve finite budgets for same-identity work.
- [x] **12.** Reserve third-party payments with exact fees and bindings.
- [x] **13.** Prepare immutable settlement effects and status records.
- [x] **14.** Commit two-party and internal settlements exactly once.
- [x] **15.** Release uncommitted holds through authorized abort decisions.
- [x] **16.** Recover Ledger participants across coordination failures.
- [x] **17.** Encode and validate canonical transfer orders.
- [x] **18.** Sign and authenticate transfers through Identity and Cryptography.
- [x] **19.** Bind transfer fee quotes to authenticated control contexts.
- [x] **20.** Commit transfers atomically with permanent replay protection.
- [x] **21.** Expose authorized transfer status and remote balance observations.
- [x] **22.** Resolve third-party payment using authenticated issuer evidence.
- [x] **23.** Encode portable claims with protected secret ownership.
- [x] **24.** Create finite claims and persist only their commitments.
- [x] **25.** Encrypt and validate owner-bound claim redemption requests.
- [x] **26.** Redeem claims atomically with owner-bound retry results.
- [x] **27.** Invalidate unused claims without weakening terminal replay guards.
- [x] **28.** Preserve obligations and replay records during bounded maintenance.
- [x] **29.** Verify races across every shared accounting resource.
- [x] **30.** Verify Ledger authority isolation and secret-safe diagnostics.
- [x] **31.** Expose bounded local Ledger inspection with discoverable help.
- [x] **32.** Complete the Ledger acceptance matrix through public contracts.
- [ ] **33.** Publish Ledger documentation and the Services handoff.

## Reference baseline and scope

Reviewed on 2026-11-06 against
[`d40b6278f24430e814d7fa82094f57261bfcac9f`](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f).
Upstream `main` is six commits ahead, affecting website/workflow/introductory
material rather than these protocol contracts; recheck before implementation.

Sources reviewed:

- [Ledger specification](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/ledger/README.md).
- [Ledger inputs](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/ledger/inputs/README.md).
- [Ledger outputs](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/ledger/outputs/README.md).
- [Ledger pseudocode](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/ledger/pseudocode/README.md).
- [LG-01–LG-13 acceptance cases](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/ledger/acceptanceTests/README.md).
- [Common module contracts and recovery](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/ModuleContracts.md).
- [Shared acceptance guide](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/AcceptanceTests.md).
- [Services coordination fault matrix](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/services/acceptanceTests/README.md).
- [Peer Protocol ledger controls and third-party payment binding](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/peer-protocol/README.md).
- [Cryptography formats and verification profile](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/cryptography/README.md).
- [Cryptography pseudocode and fixed vector](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/cryptography/pseudocode/README.md).

Ledger supplies acceptance examples and byte layouts, but no standalone
`modules/ledger/pseudocode/TestVectors.md` at this revision. Use independently
written layout fixtures and existing cryptographic vectors, labeling derived
Ledger fixtures honestly. No new confirmed protocol defect was identified in
this planning review; do not treat local storage or policy decisions as defects.

Implement under `src/modules/ledger/`, separating accounting, settlement,
transfer, claim, persistence and test responsibilities as they become useful.
Reuse public Identity/Cryptography contracts; keep composition and CLI outside
protocol internals. Every public contract needs its same-stem Markdown companion.
Use protected secret ownership from acquisition, including error/cancellation paths.

Scope is the complete direct-call Ledger participant, Linux persistence, all
payment modes through authenticated dependency ports, real cryptographic formats,
and bounded offline inspection. A durable test coordinator demonstrates Ledger's
participant behavior, not an implemented Services runtime. Peer Protocol owns
JSON controls, stream message order and live issuer queries; Services owns the
invocation coordinator and family adjustments. Later composition must supply
trusted authenticated contexts and narrowly granted authority. No new wire fields,
global currency, atomic cross-peer payment or automatic reconciliation is implied.
The minimum useful node remains incomplete after this module.

Every implementation step includes focused unit tests and relevant integration
scenarios, current contract documentation and structured logging under AGENTS.md.
Run required checks before marking a step complete, measure production coverage
using [the repository procedure](../../docs/coverage.md), and keep it above 80%.
Commit each completed step separately with its step description and meaningful
rationale/verification; record evidence in the matching section below. Planning
alone does not complete these steps or authorize implementation.

Planning validation: all 33 checklist entries have one sentence and matching
ordered detail sections; local links and whitespace checks pass. The required
formatting, Clippy, test, build and overview-help checks pass. Coverage was not
rerun because this change modifies documentation only.

## Details

### 1. Requirements and evidence

Create an assertion-level requirements matrix for LG-01–LG-13 and applicable
AT-C01–AT-C04, recording the revision used and dependencies for every row.
Resolve apparent differences by reading normative module and common contracts:
Ledger pseudocode is schematic and does not replace per-node role handling or
the prepare/decision protocol. Recheck upstream changes and existing issue
reports; report newly confirmed language-independent defects under AGENTS.md.
Do not silently choose new wire behavior. Record unresolved blockers explicitly.

Completed step 1: published the assertion-level [requirements map](../../src/modules/ledger/REQUIREMENTS.md), rechecked upstream and reconciled schematic algorithms with normative role/recovery rules. No new confirmed protocol defect. Required checks pass; documentation-only change, coverage not rerun.

### 2. Public contracts

Define local contract version 1, typed failures, trusted caller/peer contexts,
exact identities, cancellation/deadlines, opaque references and finite limits.
Bind operation keys to requester and connection/stream instances plus exact
accepted terms, roles, issuer, price mode/bound and result references; identical
input on another stream is a different attempt. Separate administrative,
coordinator, account and authenticated-control authority. Test foreign/revoked
handles, incompatible versions and invalid limits before effects.

Completed step 2: added versioned call contexts, exact operation keys, separate local authority actions, typed failures and positive admission limits. Foreign/revoked grants, wrong identity/action, over-budget calls, expired/cancelled calls and version rejection have unit tests. Required checks pass; production coverage 6832/7101 = 96.21%. Accounting transitions remain future steps.

### 3. Test dependencies

Provide reproducible A/B/C/U identities, independent literal arithmetic and
canonical bytes, controlled randomness, persistence cuts and authorized peer
receipts. Reuse real cryptographic vectors for adapter tests; fakes remain
explicitly test-only. Script dependency refusal, lost acknowledgements and
restart from durable state only. Use barriers for races, not sleeps; self-test
helper failure/exhaustion behavior. No fixture expectation comes from the codec
or arithmetic operation it tests.

Step 3 progress: added a test-only atomic register with precommit failure, lost acknowledgement, durable-image reopen and script-exhaustion tests. Focused Ledger tests and strict Clippy pass. Randomness, authenticated receipts and real-key fixtures remain pending, so this step is unchecked.

Completed step 3: Completed deterministic durable-cut, entropy, reproducible real-key identity and issuer-response helpers with refusal/exhaustion self-tests. Test-only substitutes remain excluded from production. Required checks pass; production line coverage 6832/7101 = 96.21%.

### 4. Amounts and balances

Model owned-by-issuer and issued-to-owner separately with exact uint64 values
and checked arithmetic, without self-balances or aggregated currency totals.
Represent trust exactly in [0,1] using a documented local fixed-point or rational
policy, never floating-point money. Unknown trust is zero. Expose authorized
immutable balance/available-capacity views and caller-only remote issued balance;
remote observations cannot silently replace local balances. Cover LG-01 and
LG-05 boundaries, including U64_MAX and unauthorized disclosure.

Completed step 4: Added exact issuer-separated owned/issued views, caller-only remote balance observations and local trust in integer millionths. Tests cover 100/40 separation, uint64 maximum, zero default trust, wrong authority and self-balance refusal. Required checks pass; production line coverage 6875/7144 = 96.23%.

### 5. Durability contract

Specify one authoritative writer, complete identity-bound state, transaction
serialization and durable acknowledgement. State must retain budgets, holds,
accepted terms, prepared effects, statuses, transfers, claims and payment bindings.
Distinguish known precommit failure from possible commitment with lost reply;
uncertainty gates affected work until reload/status resolves it. Implement a
volatile test adapter and failure tests before dependent mutations, never claim
volatile state is durable. Choose and document a bounded storage representation
and its schema/version migration refusal rules.

Completed step 5: Defined complete identity-bound durable image transactions, precommit versus unknown failure semantics and an explicitly volatile copy adapter. Tested missing versus present state and immutable image replacement; runtime recovery will consume these ports in later steps. Required checks pass; production line coverage 6884/7153 = 96.24%.

### 6. Linux adapter

Implement a Linux owner-only store with no-follow path handling, exclusive
writer coordination, atomic transactions and required synchronization before
acknowledgement. Choose an engine deliberately; keep dependencies and Cargo.lock
reviewable. Reuse public platform contracts rather than Board/Identity private
storage. Validate ownership, identity, bounds, format and accounting invariants
on load; distinguish explicit initialization from opening a missing/corrupt store.
Test real reopen, permissions, symlinks, contention and storage failure; unsupported
platforms return explicit errors. Paid Storage is not a dependency.

Completed step 6: Implemented Ledger-owned Linux descriptor-anchored protected persistence, explicit directory creation, exclusive writers, bounded versioned framing and synchronized atomic replacement. Real-store tests cover reopen, contention, corruption, symlinks, permissions and all five simulated durable cuts. Required checks pass; production line coverage 7052/7332 = 96.18%.

### 7. Administration

Add explicit finite issuance/internal budgets, trust updates and bounded grants
and revocations through local administrator authority. Define owned-balance
initialization/reconciliation as an explicit authorized local policy, not a peer
claim or hidden query side effect. Persist changes before success. Never create
self-balances, implicitly increase trust or reset spent budgets on reopen.
Protect existing holds and accepted obligations when changing policy or revoking
credits. Cover LG-05/LG-06 and denial of every administrative call from peer context.

Completed step 7: Implemented serialized complete-image accounting with finite administrative grants, revocation, explicit owned reconciliation and exact trust. Tested authorization, self-account refusal, exhausted budgets, durable reopen, corrupt binding rejection and unknown-commit gating/reload without reapplying changes. Required checks pass; production line coverage 7330/7619 = 96.21%.

### 8. Mode selection

Derive provider-issued, requester-issued, third-party and same-identity modes
from explicit requester/provider/issuer IDs, independent of stream opener.
Each node updates only its own view: check provider trust and credit acceptance
where that node has authority; a requester cannot fabricate the provider's trust.
Require finite spending/issuance authority and explicit third-party risk policy.
Reject impossible local roles, zero prices, conflicting fixed/metered terms and
unsupported adjustments before reservation; existing paid entitlements do not
create zero-price service reservations. Test each role and refusal separately.

Completed step 8: Added immutable accepted terms and explicit issuer-based payment mode assessment, independent of TCP opener. Tests cover all roles, zero trust, missing payment fields, optional mode policy refusal, total fee bounds and uint64 overflow; requester context cannot assert provider trust. Required checks pass; production line coverage 7368/7658 = 96.21%.

### 9. Provider-issued holds

On the requester reserve owned provider credits; on the provider reserve its
recognized issued credits for the requester. Persist the full operation binding
and one opaque hold without changing the underlying balance. Exact retries reuse
the hold and conflicting parameters fail. Test LG-02, the 100/60 competing-hold
case, insufficient capacity, U64_MAX and the absence of cross-ledger mutation.
Serialization must account for all other withdrawals from the same balance.

Completed step 9: Added durable operation-bound provider-credit holds in each local role, exact retry/conflict handling and available-balance observations. Tests cover competing 60/100 reservations, unchanged balances, restart retention and administrative protection of held capacity. Required checks pass; production line coverage 7548/7867 = 95.95%.

### 10. Requester-issued holds

Reserve finite issuance on the requester and checked incoming balance headroom
on the provider, with explicit acceptance of requester credits. Hold destination
headroom against competing additions so accepted work cannot overflow later.
No owned self-currency is required. Exercise LG-03 price7/budget10, concurrent
incoming credits, denied issuer policy and exact retry/conflict behavior.

Completed step 10: Implemented requester-issued reservations with finite issuance, protected destination headroom and provider-side incoming credit holds. Tests demonstrate budget10/price7 exhaustion and refusal of administration that would consume reserved capacity. Required checks pass; production line coverage 7572/7891 = 95.96%.

### 11. Internal resource use

Use the coordinated local identity writer and a separate finite internal budget
for positive-price same-identity operations. Persist reservations and eventual
resource use without creating external credits or self-balances. Test exhaustion,
concurrent attempts, restart and replay; copying keys never creates another
independent allocation of the same budget.

Completed step 11: Implemented finite same-identity internal holds without self-balances or external earnings. Tests reject a second price7 reservation from budget10 and preserve underlying budgets until settlement; local coordinator authority authorizes internal work. Required checks pass; production line coverage 7575/7892 = 95.98%.

### 12. Third-party holds

Bind (issuer, requester, transferId) to exactly one invocation and preserve that
binding durably. Require a fresh transfer identity, explicit risk acceptance and
exact agreed fee; checked maximumPrice + fee must fit uint64 and the authorized
total spending bound. Reserve requester-owned issuer credits and appropriate
provider incoming headroom under local policy. Test overflow, payment reuse,
unsupported policy and both roles without initiating a network transfer.

Completed step 12: Added exact fee-inclusive third-party holds and permanent per-invocation payment tuple binding. The price7/fee2 case reserves9 from100 and rejects reuse on another stream; local role, risk policy and overflow checks precede admission. Required checks pass; production line coverage 7590/7907 = 95.99%.

### 13. Prepare and status

Prepare only from the matching reservation with complete validated result/effect
references, final price equal to fixed price or within the metered bound, and
separately authorized contract adjustments. Return a durable owner/key/effect-bound
token; exact retries return the same association, conflicts have no effect.
Expose absent/prepared/committed/aborted status without protected-data leaks;
represent reserved-but-not-prepared state explicitly in the local API. Prepared
holds survive deadlines, cancellation and dependency loss. No generic refund port.

Completed step 13: Added exact final-price/result-reference preparation, opaque effect tokens and durable status queries. Tests cover absent/reserved/prepared state, price/ref conflicts, bound rejection, exact retry, held funds and restart without releasing prepared capacity. Required checks pass; production line coverage 7658/7978 = 95.99%.

### 14. Local commit

Accept only the authorized coordinator's immutable commit decision and exact
prepared token. Atomically apply the role-specific debit, issuance, incoming
credit or internal use with status/result retention and unused metered capacity
release. Test LG-02–LG-05: price7, 60 reserved/21 final, exact maximum, duplicate
commit, wrong token and lost reply. Preserve accepted terms across trust/policy
changes; no assertion of atomicity between independent peers or service stores.

Completed step 14: Implemented token-bound local commits for provider/requester credits and internal resource use, exact retry idempotence and unused metered release. Mode-by-mode tests assert one price21 effect from a maximum60 reservation; third-party completion retains an unconfirmed payment hold. Required checks pass; production line coverage 7700/8029 = 95.90%.

### 15. Abort semantics

Allow explicit failure before preparation to release a hold and coordinated
abort to release a prepared effect when no commit decision exists. Persist abort
and preserve idempotent/conflicting-key history. Refuse abort after commit and
never infer rollback from timeout, socket loss or missing coordinator. Test each
state transition and races with commit, including revoked new-work authority
without orphaning authorized recovery of accepted obligations.

Completed step 15: Added explicit precommit failure release and exact-token coordinated abort. Tests cover repeated abort, prepared calls requiring a token, cancelled calls retaining holds, commit-after-abort conflict and refusal to undo committed effects. Required checks pass; production line coverage 7730/8061 = 95.89%.

### 16. Local recovery

Run Ledger as the real durable participant with a test-owned coordinator and
handler implementing SV-09. Cut before/after reservation, prepare, decision,
participant commit and acknowledgement; recover only from durable data and retry
recorded decisions, never service execution. Gate conflicting reads/spends while
unresolved. Include fresh-process reopen and distinguish simulated crash evidence
from filesystem/power-loss guarantees. Real Services coordination remains later
integration, explicitly identified under LG-13.

Completed step 16: Exercised the Ledger participant across eight coordinator decision cuts with recovered decision delivery, exact replay and consistent price7 accounting. Added a separate-process protected-store reopen scenario; simulated handler/coordinator coverage is explicitly distinguished from future real Services integration. Required checks pass; production line coverage 7731/8061 = 95.91%.

### 17. Transfer bytes

Implement exactly 121 unsigned bytes: version1, transferId16, issuer32, owner32,
newOwner32 and positive uint64be amount at offsets 1/17/49/81/113. Reject the old
105-byte layout, invalid versions/lengths, trailing bytes, zero and forbidden
recipient identities. Keep public key/signature external to signed bytes.
Use independent literal LG-07 bytes and all applicable LG-08 mutations; canonical
JSON controls belong to Peer Protocol, not this binary codec.

Completed step 17: Implemented exact 121-byte version-one transfer encoding and strict decoding with independent literal offset/amount fixtures. Tests reject the old105-byte layout, every truncated prefix, trailing data, unknown versions, zero amount and forbidden recipients. Required checks pass; production line coverage 7759/8089 = 95.92%.

### 18. Transfer authority

Create fresh cryptographically random transfer IDs under explicit local spending
and signing authority; use protected Identity operations and the existing strict
Ed25519 profile. Verify authenticated caller=owner, derived key identity=owner,
issuer=local and signature over exact bytes. Test real signatures, mixed or weak
key rejection through the shared verifier, wrong purpose, entropy/signing failure
and cancellation. Never expose keys or treat a signature alone as credit authority.

Completed step 18: Added protected Identity signing adapter, explicit outgoing spending authorization, cryptographic transfer IDs and strict owner/key/signature verification. Real Ed25519 tests reject modified bytes, wrong callers/issuers, weak keys, truncated signatures, entropy exhaustion and revoked spending authority. Required checks pass; production line coverage 7813/8149 = 95.88%.

### 19. Fee agreement

Quote a finite exact fee with 0 <= fee < amount and fee <= maximumFee through
explicit issuer policy. Retain a bounded quote reference bound to owner, order,
connection/stream and exact fee; acceptance on another context or with changed
fee fails. Quotes alone do not debit or guarantee capacity. Already committed
exact retries bypass requoting and later fee changes. Test zero fees, exhaustion,
expiration/disconnect before acceptance and every fee-related LG-08 row.

Completed step 19: Added bounded transient fee quotes with strict signed-owner validation, exact fee ceilings, monotonic expiry and same-stream acceptance. Real-signature tests reject altered fees, stale/foreign contexts, duplicates and capacity overflow; acceptance exposes no issuer authority or balance mutation. Required checks pass; production line coverage 7919/8257 = 95.91%.

### 20. Issuer transfer effects

Under one transaction recheck replay, spendable source balance and recipient
headroom, subtract gross amount, add net amount and persist exact order/result.
Use (owner, transferId) as the lifetime replay key. Exact retries return the
original result; changed signed bytes conflict. Test 100/9/2 -> 91/7, simultaneous
acceptances, interaction with service holds, overflow and failure cuts. A failed
transfer changes no balances; retained replay data cannot be evicted to admit work.

Completed step 20: Implemented atomic issuer transfer debits/net credits and permanent owner/transfer replay records. Tests assert gross9/fee2 produces balances91/7, exact duplicate/restart returns the original fee/result, changed orders conflict and full replay capacity refuses new transfers. Required checks pass; production line coverage 8004/8372 = 95.60%.

### 21. Queries and reconciliation

Return original transfer results only to original/new owners and bind issuer
provenance to trusted connection context, not JSON fields. Query remote balances
as explicit observations with no automatic reconciliation of in-flight local
accounting. Test unrelated callers, missing keys, exact values, issuer refusal,
restart and lost transfer replies. Do not turn status recovery into a new transfer
or service invocation; network request execution remains composition's job.

Completed step 21: Added owner/new-owner-only transfer status and caller-scoped issuer balance queries. Tests deny unrelated callers for both existing/missing results, deny wrong action grants and preserve exact91/7 accounting without hidden reconciliation or network retries. Required checks pass; production line coverage 8032/8402 = 95.60%.

### 22. Payment resolution

After valid local work, preserve service completion separately from pending,
unsettled, unknown and settled payment. Emit a bounded transfer/query request
through an explicit composition port; validate original issuer authentication,
owner, recipient, transferId, gross amount, agreed fee and final net price before
one local accounting effect. Bind evidence to the original operation and reject
reuse, forwarded requester JSON and changed fees. Test every LG-10 row, both
local roles, persisted refusal/uncertainty, duplicate evidence and restart without
re-executing work. Never assume issuer refusal rolls back successful service work.

Completed step 22: Implemented separately authorized original-issuer receipts, exact per-invocation payment matching and explicit transfer/query action requests. Tests reject forwarded-control authority and wrong owner/recipient/ID/amount/fee, preserve unknown/refused states, and settle both local roles once across restart. Required checks pass; production line coverage 8176/8546 = 95.67%.

### 23. Claim format

Implement cr2se-claim: plus canonical padded RFC4648 Base64 of exactly 105 bytes:
version1, issuer32, claimId32, secret32 and positive uint64be amount. Reject wrong
versions/lengths, nonzero unused bits, whitespace and zero amounts. Keep bearer
material in guarded secret types without revealing Debug/Display; explicitly
controlled export transfers secret ownership. Test literal layout and malformed
encoding independently of production round trips.

Completed step 23: Implemented guarded portable105-byte claims with strict canonical RFC4648 text and explicit private export. Independent literal tests pin issuer/claim/secret/amount layout and reject aliases, invalid alphabet, wrong lengths/padding, unsupported version and zero amount; no automatic diagnostic/serialization exposes bearer material. Required checks pass; production line coverage 8232/8602 = 95.70%.

### 24. Claim creation

Require explicit administrator authority, independently generated random 32-byte
claim ID and secret, unique ID and finite issuance reservation. Persist SHA256
commitment, authoritative amount, restrictions and AVAILABLE state before return;
do not store the clear secret. Test LG-11 amount7/budget20, collisions, entropy
failure, overflow, revoked authority and commit uncertainty. Lost export does not
authorize duplicate issuance: expose recoverable status/invalidation for the
bound creation attempt and explain that the secret cannot be recovered from its
commitment.

Completed step 24: Implemented independently randomized finite claim creation with guarded secret ownership, commitment-only records, recipient restrictions and durable administrative creation keys. Tests verify7 reserved from20, restart retention, grant exclusion, duplicate creation refusal, denied authority and entropy loss. Required checks pass; production line coverage 8373/8756 = 95.63%.

### 25. Redemption envelope

Encode exact ASCII CR2SE-CLAIM-REDEEM-V1 followed by issuer/owner/claimId/secret,
each32 bytes, with no terminator or amount. Reuse validated persistent recipient
credentials and generic identity encryption; Transport alone is not confidential.
Decrypt through protected identity authority and release plaintext only after
authentication, then validate length/domain/local issuer/authenticated owner.
Test real encryption, tampering, wrong credential/recipient, truncation, resource
bounds and cleanup on all failures; no secret in diagnostics or wire plaintext.

Completed step 25: Implemented exact owner/issuer/domain-bound generic encrypted claim requests and protected Identity decryption adapter. Real envelope tests reject ciphertext/header tampering, wrong owner and truncation; secret plaintext stays guarded and is absent from transmitted objects. Required checks pass; production line coverage 8431/8830 = 95.48%.

### 26. Claim redemption

Compare SHA256(secret) with the stored commitment in constant time, check claim
state and restrictions, consume reserved issuance and add the stored authoritative
amount under one transaction. Reserve/check recipient headroom; another operation
cannot make a successful credit overflow. Same secret and original owner replay
the original result without issuance; another owner fails. Test all LG-11/LG-12
rows, concurrent redeemers, zero-trust control access and lost reply/restart.
Portable amount cannot replace stored amount or implicitly grant service trust.

Completed step 26: Implemented constant-time commitment-checked claim redemption with atomic reserved issuance consumption, destination headroom and owner-bound original results. Real encrypted tests cover wrong secrets/owners, portable amount99 versus stored7, zero-trust bootstrap, exact retries and restart without duplicate issuance. Required checks pass; production line coverage 8490/8886 = 95.54%.

### 27. Claim lifecycle

Allow only authorized invalidation of AVAILABLE claims, releasing their finite
issuance reservation atomically. Retain invalidated/redeemed records and reject
recreation of their IDs; no expiry invented for bearer claims. Test invalidate
versus redeem in both serialization orders, repeated local calls, restart and
rejected attempts against terminal records without exposing bearer information.

Completed step 27: Implemented authorized claim invalidation with issuance release and retained terminal records. Real encrypted redemption/invalidation races admit one winner; tests cover repeated invalidation, redeemed-claim refusal and invalidated replay rejection after restart. Required checks pass; production line coverage 8504/8897 = 95.58%.

### 28. Capacity and compaction

Bound new accounts, operations, quote state, claims, transfers and stored bytes;
refuse new admission before losing required records. Define safe accounting/history
compaction only after coordinator and retry obligations permit it. Preserve ledger
lifetime transfer guards and terminal claims, accepted terms and unresolved holds.
Test near-capacity writes, growing replay sets, interrupted maintenance, schema
version refusal and recovery with smaller configured limits. Never reset replay
history while retaining balances or silently discard durable obligations.

Completed step 28: Added conservative zero-balance maintenance that retains all obligations and lifetime replay records, strict recovered role/effect/payment invariants and admission bounds. Tests reject smaller reopen limits without rewriting data and verify metered third-party holds survive restart at the final gross amount. Required checks pass; production line coverage 8602/9002 = 95.56%.

### 29. Concurrency invariants

Use barriers to race reservation/settlement/transfer/grant/revoke/claim operations
against the same sources, destinations and budgets in both orders. Assert checked
balances, finite capacity, headroom, unique payment bindings and no duplicate
effects after retries. Test writer contention across processes and isolation
between identities. Validate recovered invariants and explicitly document any
conservative local rejection policy instead of implying distributed consensus.

Completed step 29: Added barrier races for shared issued sources, transfers versus holds, revocations versus holds, finite issuance versus grants and duplicate commit delivery. Added separate-process writer contention; existing incoming-headroom and real claim invalidation races complete the shared-resource evidence. Required checks pass; production line coverage 8602/9002 = 95.56%.

### 30. Security and logging

Exercise forged contexts/tokens, revoked capabilities, mismatched owners,
contract versions and cancelled/expired calls at effect boundaries. Capture
structured TRACE logs on success and failure; exclude claim secrets, private keys,
raw envelopes and arbitrary peer text, and bound repeated diagnostic volume.
Use real Identity/Cryptography failure paths and prove missing dependencies cause
explicit failure without hidden network, signing, grants or retries.

Completed step 30: Added forged-context/token, revocation and cancellation isolation tests plus bounded TRACE payload/secret exclusion. Real protected Identity storage exercises signing/decryption adapters and byte-grant failures; dependency categories remain explicit, per-byte decode logging is avoided and blocked reads recheck authority. Required checks pass; production line coverage 8655/9039 = 95.75%.

### 31. Local exercise commands

Add authorized read-only ledger-show with explicit store/local identity/account
selection and exact issuer-separated output, including pending/unknown status.
Never initialize or repair a store as a side effect of inspection or print bearer
material. Define spellings/defaults before wiring; keep parser, overview, every
help <topic> alias, README and executable tests synchronized. Errors go to stderr
with syntax2/runtime1; no arguments still show successful help. Exercise mutations
through runnable library scenarios until deliberate administration composition
exists; do not add an unsafe peer-facing admin or secret-on-command-line interface.

Completed step 31: Added bounded existing-store ledger-show with explicit local/account identities, exact issuer-separated output and aggregate pending/unknown payment status. Shared help covers every new spelling; executable tests cover help/syntax, contention, uint64 maximum, wrong owner, corruption and no implicit initialization. Required checks pass; production line coverage 8752/9150 = 95.65%.

### 32. Acceptance audit

Execute all LG-01–LG-13 direct-call rows and applicable AT-C01–AT-C04 with literal
expected values, real cryptography and persistence where claimed. Map each matrix
mutation to an assertion; record policy refusals separately from supported positive
paths. LG-13 uses the real Ledger participant with the simulated coordinator from
step16; full Services SV-09 and Peer Protocol network controls remain named later
integration obligations. Confirm new protocol issues are linked/reported and
upstream fixes verified; do not mark missing integration or vectors as passed.

Completed step 32: Completed assertion-level LG01–LG13 and shared-contract evidence with literal two-party/internal price7, exact uint64 spending/headroom, finite grant5 bootstrap, real signed fee acceptance/replay, zero-fee transfer and independent claim-hash oracle tests. Documented direct-call/Linux scope and deferred real Services/network integration without claiming node conformance. Required checks pass; production line coverage 8779/9150 = 95.95%.

### 33. Completion and handoff

Finish module README, public companion documents, acceptance evidence and runnable
build/test/library/inspection examples from repository root. Explain authority,
limits, issuance policy, local-versus-remote uncertainty, protected persistence,
recovery and replay retention. Run all AGENTS.md checks, API docs, changed CLI topics
and the production coverage procedure; require >80% and record the actual result.
Mark only implemented scope complete in this plan/roadmap and root README, list
remaining composition work, and hand off to Services without expanding its plan.
Commit focused completed steps with descriptive titles and bodies after checks.
