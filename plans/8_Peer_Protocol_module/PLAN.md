# Peer Protocol module plan

Status: implementation in progress; checked steps record verified work.

This is roadmap milestone9 and planning folder8, following Services.

## Steps

- [x] **1.** Reconcile Peer Protocol requirements with existing module contracts.
- [x] **2.** Define authenticated exchange contexts and finite limits.
- [x] **3.** Add deterministic stream and dependency helpers.
- [x] **4.** Import pinned independent wire fixtures.
- [x] **5.** Decode bounded records across Transport frames.
- [x] **6.** Encode records with bounded outbound buffering.
- [x] **7.** Parse lossless bounded control objects.
- [x] **8.** Validate control fields and fixed identifiers.
- [x] **9.** Bind exact integer and floating-point service values.
- [x] **10.** Implement canonical inline byte values.
- [x] **11.** Assign deterministic detached body descriptors.
- [x] **12.** Receive ordered bodies through bounded ownership.
- [x] **13.** Stream outgoing bodies in descriptor order.
- [x] **14.** Implement exchange ordering and terminal states.
- [x] **15.** Serve bounded Board and definition requests.
- [x] **16.** Consume metadata with authenticated publisher provenance.
- [x] **17.** Authorize provided invocation proposals.
- [x] **18.** Construct exact stream-bound service quotes.
- [x] **19.** Validate requester quotes and explicit spending consent.
- [x] **20.** Bind acceptance to quote bytes and one reservation.
- [x] **21.** Deliver complete input only after its terminal markers.
- [x] **22.** Transmit durably committed service results.
- [x] **23.** Accept results only after normal stream completion.
- [x] **24.** Preserve failure codes and uncertain outcomes.
- [x] **25.** Implement wanted-offering role reversal.
- [x] **26.** Bind paid checks to challenge-only evidence.
- [x] **27.** Reconcile included allowances with complete evidence delivery.
- [x] **28.** Add explicit family-authorized cached retry handling.
- [x] **29.** Bind authenticated balance and transfer-query controls.
- [x] **30.** Implement transfer quote and acceptance exchanges.
- [x] **31.** Implement bounded claim-redemption exchanges.
- [x] **32.** Verify third-party settlement on independent issuer streams.
- [ ] **33.** Connect the engine to real Transport composition.
- [ ] **34.** Verify resource isolation and concurrent exchanges.
- [ ] **35.** Exercise complete exchanges between real local nodes.
- [ ] **36.** Add discoverable runtime configuration and exercise instructions.
- [ ] **37.** Complete the Peer Protocol acceptance matrix.
- [ ] **38.** Publish the completed module and Storage handoff.

## Baseline, scope and review

Reviewed on 2026-11-10 against revision `d40b6278f24430e814d7fa82094f57261bfcac9f`. Upstream main remains six commits ahead with website/workflow/introductory changes only; the reviewed protocol files are unchanged. Recheck before implementation.

- [Specification](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/peer-protocol/README.md).
- [Inputs](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/peer-protocol/inputs/README.md).
- [Outputs](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/peer-protocol/outputs/README.md).
- [Pseudocode](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/peer-protocol/pseudocode/README.md).
- [Acceptance cases PP-01–PP-13](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/peer-protocol/acceptanceTests/README.md).
- [Fixed encoding vectors](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/peer-protocol/pseudocode/TestVectors.md).
- [Common contracts](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/ModuleContracts.md).

Use the [Services handoff](../../src/modules/services/ACCEPTANCE.md),
[Transport API](../../src/modules/transport/api.md), [Board guide](../../src/modules/board/README.md)
and [Ledger guide](../../src/modules/ledger/README.md). Peer Protocol owns wire
framing, ordering and authenticated exchange association; Services owns local
coordination, Ledger owns balances and families own resources. Keep protocol code
under `src/modules/peer_protocol/`, organized into focused codec, value, body and
exchange responsibilities as needed; application composition stays outside it.
Use public contracts and add narrowly scoped prerequisite changes to existing
modules with regression tests when integration exposes missing capabilities.

This plan includes real Transport integration and explicit local custom-handler
exercises. Paid Storage and optional family implementations remain separate work;
family-specific wire cases use explicit test-owned contract substitutes until those
providers exist. Never advertise absent capabilities. The Services local adapter
is not a peer wire codec, and local send acknowledgement is not remote receipt.

Existing [issue10](https://github.com/beothorn/CR2SE/issues/10) remains relevant to
paid-check timeout after provider commitment; do not claim globally zero-charge
timeouts while unresolved. The accepted-attempt allowance in current Services
also requires an implementation audit against Peer Protocol's complete-response
consumption rule. That observed local integration mismatch is not by itself a
new protocol defect. No new confirmed language-independent defect is filed by this
planning change. Search open and closed upstream issues before reporting a newly
confirmed contradiction, retain local evidence and confirmed issue URLs under
[CR2SE bugs](../../CR2SE%20bugs/README.md), and never invent interoperable behavior.

Every implementation step includes unit tests, invalid/boundary/dependency cases,
current public Markdown companions, function comments and safe structured logging.
Run applicable integration tests and required AGENTS.md checks before checking it
off; measure production coverage under [the documented procedure](../../docs/coverage.md)
and keep it above80%. Commit focused verified changes with descriptive titles and
bodies, then push under the standing workflow. Later acceptance work consolidates
evidence rather than postponing tests. The original planning change left every step unchecked; the subsequent user request authorizes implementation of all38 steps.

## Details

### 1. Reconcile Peer Protocol requirements with existing module contracts

Map PP-01–PP-13 and shared contract cases to observable assertions and dependencies; recheck upstream and issue10 before implementation. Audit Transport events, Board provenance, Services acceptance/result ownership and Ledger grants. Record necessary public-port changes without accessing private state.

### 2. Define authenticated exchange contexts and finite limits

Define local authority, contract version, trusted connection/stream identity, caller deadlines and cancellation. Separate opener from requester/provider. Bound connections, exchanges, queued bytes, descriptors, temporary bodies and control rate; test foreign, revoked and exhausted contexts.

### 3. Add deterministic stream and dependency helpers

Provide fragmented/coalesced DATA, half-closes, cancellations, backpressure, clock and entropy scripts plus Board/Services/Ledger substitutes. Count dispatches and financial effects, inject lost replies, and fail on script exhaustion without sleeps.

### 4. Import pinned independent wire fixtures

Retain exact upstream control and binary record bytes and typed-value vectors with revision/source provenance. Add independently calculated quote hashes and malformed variants; distinguish these from the Services local codec and Transport authentication vectors.

### 5. Decode bounded records across Transport frames

Implement incremental five-byte framing, checking kind/length before allocation; support control1..1048576 and binary record13..65548. Test every split, single-byte input, coalescing, truncation and maximum-size records under PP-01/02.

### 6. Encode records with bounded outbound buffering

Emit complete logical records across arbitrary DATA limits and backpressure without duplication. Enforce total queued budgets, cancellation and ownership; test slow consumers and partial writes independently of framing decode.

### 7. Parse lossless bounded control objects

Reject duplicate members, trailing JSON, malformed UTF-8, null substitutes, missing type and depth33; preserve exact numeric lexemes and original control bytes. Accept permitted depth32 and full-size controls; preserve document uint64 numbers without binary64 coercion.

### 8. Validate control fields and fixed identifiers

Define typed metadata, invocation, failure and Ledger messages with required/optional fields, canonical identity text, exact fixed byte lengths and uint32 rules. Keep unknown optional members inert and diagnostic strings bounded to1024 UTF-8 bytes.

### 9. Bind exact integer and floating-point service values

Implement schema-directed canonical integer strings, int32/uint32 integer tokens, IEEE lowercase bit objects and canonical NaNs only with contract permission. Cover all PP-03 extrema, signed zero and invalid spellings; do not reuse the local Services codec as wire encoding.

### 10. Implement canonical inline byte values

Validate padded RFC4648 Base64, zero unused bits and the4096 decoded-byte inline maximum. Preserve signed/encrypted objects byte-for-byte; test empty,4096,4097 and malformed padding/whitespace variants.

### 11. Assign deterministic detached body descriptors

Traverse object names by unsigned UTF-8 bytes and arrays by index, assigning unique sequential uint32 IDs from zero. Enforce exact descriptor members, canonical lengths, inline empty values and overflow bounds; support nonempty small detached values allowed by vectors.

### 12. Receive ordered bodies through bounded ownership

Validate declared ID order, contiguous offsets, exact lengths and final UTF-8 before publishing complete values. Use Services Body/public storage ports where compatible; bound temporary storage and cleanup, and test gaps, overlaps, missing/extra bytes and interrupted ownership.

### 13. Stream outgoing bodies in descriptor order

Retain immutable sources until transmission completes, chunk data at1..65536 bytes and obey Transport backpressure. Test source failure, cancellation, byte offsets and result buffers surviving caller-session loss.

### 14. Implement exchange ordering and terminal states

Define explicit metadata, invoke/check, wanted and Ledger state machines for both endpoints. Track direction-specific END, markers and abnormal close; reject illegal transitions with stream-scoped PROTOCOL_ERROR while other streams remain usable.

### 15. Serve bounded Board and definition requests

Use public Board snapshots and current offering definitions; ensure the complete wrapped response fits the control maximum before publication/use. Missing offerings fail without synthetic definitions; no binary bodies or service charge, END both directions, and control quotas apply.

### 16. Consume metadata with authenticated publisher provenance

Attribute responses only to the authenticated publisher and correct request, preserving lossless documents and cache/definition associations. Test stale revisions, forwarding, wrong offerings and exact response-size boundaries with actual Board contracts.

### 17. Authorize provided invocation proposals

Decode explicit issuer, maximum, mode and optional third-party binding against current selection and trusted peer context. Validate metadata before bodies; absent capabilities, unknown preconditions and malformed economics must cause no execution or charge.

### 18. Construct exact stream-bound service quotes

Inject entropy for fresh32-byte quote IDs, retain exact transmitted JSON, include immutable offering/issuer/mode/factors and permitted transfer/release fields. Test entropy failure, finite quote lifetime and complete response sizing without reserving merely on quote creation.

### 19. Validate requester quotes and explicit spending consent

Independently recalculate fixed/metered prices and provider size factors, require compatible definition identifiers/version/description, and enforce maximum plus fee. Price changes need explicit local consent; contract changes require a fresh definition exchange.

### 20. Bind acceptance to quote bytes and one reservation

Verify original quote ID, SHA256 of exact JSON bytes, peer, stream, current terms and capacity. Adapt public Services admission so requester reservation precedes sending accept and provider reservation follows valid receipt; lost sends retain uncertainty and duplicate acceptance cannot double-reserve.

### 21. Deliver complete input only after its terminal markers

Allow bodies only after acceptance and require exact content, input.end and requester END before effectful dispatch. Test incomplete UTF-8/content hashes, premature bodies, invalid ordering and no charge/execution on invalid input.

### 22. Transmit durably committed service results

Use Services retained output and final price after required local decisions, then send metadata, bodies, result.end and END. Preserve journal/result ownership on failed sends; verify fixed/metered price and any output price/finalPrice field agree with the envelope.

### 23. Accept results only after normal stream completion

Keep received metadata and bodies provisional until exact completion, result.end and normal END. Validate schema, quoted sizes, content and price through Services; partial data and unauthenticated plaintext cannot become successful application output.

### 24. Preserve failure codes and uncertain outcomes

Map typed failures to specified codes without leaking concealed resource existence; unknown codes remain generic failures. Discard provisional output on permitted failure, reject failure after result.end, and retain possible obligations after disconnect/CANCEL without remote job-status or automatic work retries.

### 25. Implement wanted-offering role reversal

Require locally installed prospective provider support before offer; obtain publisher input and spending authority from local application policy. Exercise offer/propose/quote/accept with requester opposite the opener, refusal and malicious input/installation suggestions.

### 26. Bind paid checks to challenge-only evidence

Use check input and family-specific evidence shapes, never provider-computed correctness outcomes or transmitted expected bytes. Start response timing at input.end, classify late/missing evidence and refuse local-only checks. Revisit issue10 and document any unresolved global charging limitation.

### 27. Reconcile included allowances with complete evidence delivery

Audit and adjust the existing Services accepted-attempt quota through public contracts to distinguish in-flight reservation from consumed allowance. Match the pinned Discovery rule that only a complete evidence response consumes its allowance; test concurrent attempts, failure, durable restart and response-loss ambiguity without silently resetting guards.

### 28. Add explicit family-authorized cached retry handling

Define a narrow public capability for validated Messaging recovery/removal replay, checking original caller and all request fields before current prices/balances. Return original cached output/price directly only for a matching proposal, with no new quote/body/charge; absent families refuse and arbitrary early results fail.

### 29. Bind authenticated balance and transfer-query controls

Decode ledger.balance and ledger.transfer.get on independent streams with correct caller scopes and exact values. Query access is limited to original/new owner; return specified response and END without dispatching Services or granting administrative authority.

### 30. Implement transfer quote and acceptance exchanges

Validate121-byte order,32-byte public key,64-byte signature, transfer ID and exact maximum fee through Ledger/Identity contracts. Require exact fee acceptance before commitment and END; accepted retries return original results without requoting or duplicate effects.

### 31. Implement bounded claim-redemption exchanges

Decode the encrypted envelope and invoke authorized Ledger/Identity redemption ports with authenticated redeemer context. Preserve exact claim ID/amount, owner-specific replay and failure semantics; test malformed/oversized ciphertext and unavailable dependencies without exposing claim secrets.

### 32. Verify third-party settlement on independent issuer streams

Bind one transfer to one invocation and obtain original-issuer authenticated evidence for owner/newOwner/ID/gross/fee/net. Reject requester-forwarded receipts and reused payments; fee changes, refusal or unreachable issuers leave payment unsettled/unknown without reversing service work.

### 33. Connect the engine to real Transport composition

Register cr2se.peer version1 through supported Transport APIs, translate verified events/actions and preserve unique connection instances. Route asynchronous dependencies without blocking the reactor or bypassing module authority; test negotiation refusal and isolated stream failures with real adapters.

### 34. Verify resource isolation and concurrent exchanges

Race acceptance, cancellation, body completion, handler drain and settlement using barriers and counters. Exercise rate/byte/time quotas, slow readers, fairness and cross-stream/identity isolation; capture bounded TRACE output excluding bodies, quotes containing secrets and claim material.

### 35. Exercise complete exchanges between real local nodes

Use actual Identity, Cryptography, Transport, Board, Services and Ledger with explicitly installed custom test handlers. Cover fixed work, wanted roles, bodies, metadata, checks and controls; use a third issuer for settlement and restart/loss cases. Test real processes where claimed and distinguish same-implementation integration from interoperability.

### 36. Add discoverable runtime configuration and exercise instructions

Document explicit module/store initialization, recovery, supported capabilities and a reproducible two-node custom-service exercise. Add only necessary composition/CLI controls, update overview and detailed help for every spelling, and test exit statuses; standard-family providers remain separate milestones.

### 37. Complete the Peer Protocol acceptance matrix

Map each PP-01–PP-13 assertion and applicable shared/BD-09/LG-13/Services handoff case to executable tests and independent vectors. Label optional-family substitutes, unresolved protocol questions and process/network evidence precisely; recheck upstream and report confirmed language-independent defects without duplicates.

### 38. Publish the completed module and Storage handoff

Finish companion contracts, module/root guides, roadmap status and failure/recovery limits. Run all required checks, changed help/examples, warning-free API docs and measured production coverage above80%; mark only verified scope complete and list paid Storage integration obligations.

## Planning validation

Reviewed all38 one-sentence checklist items against the13 upstream acceptance
cases, input/output contracts, pseudocode, vectors and existing Services handoff.
All items remain unchecked with matching numbered details in order. Local links
and whitespace validate. Formatting, strict all-target Clippy, locked tests,
build and overview help pass. Coverage was not rerun because only planning and
navigation documents changed; implementation must measure it as described above.

Step1: reviewed unchanged upstream contracts and published the requirements/port audit in [REQUIREMENTS.md](../../src/modules/peer_protocol/REQUIREMENTS.md). Detached metadata, price-envelope equality, allowance consumption and exact acceptance ordering require explicit integration work.

Step2: added independent Peer Protocol authority, scoped grants, exact operation identities, version/deadline/cancellation contexts and finite connection/exchange/body/quote/node/queue/control budgets with unit tests. Contract declarations do not advertise wire dispatch. Required checks pass; measured production coverage12046/12774 =94.30%.

Step3 completed: test-only fragmented stream/terminal scripts, bounded sends and counted generic dependency/entropy replies fail closed on exhaustion. Required checks pass; production coverage12050/12774 =94.33%. Corrupt generated Cargo metadata was removed and verification rerun successfully.

Step4 completed: pinned literal control/binary and canonical typed-value fixtures with upstream revision/blob provenance and a separately calculated exact-source quote hash. Fixture tests and required checks pass; no production code changed, so step3 production coverage remains applicable.

Step5 completed: bounded incremental framing validates headers before allocation, preserves exact control bytes and rejects truncation/invalid kinds/lengths. Independent vectors, all split positions, bytewise/coalesced delivery and maximum-sized records pass. Required checks pass; production coverage12121/12850 =94.33%.

Step6 completed: aggregate outbound admission checks encoded size before allocation, borrowed fragments persist under backpressure, and explicit partial acknowledgements preserve record order. Cancellation drops only unsent local bytes. Required checks pass; 12184/12914 = 94.35%.

Step7 completed: wire-specific lossless control parsing preserves exact source/large integers and rejects duplicate decoded keys, malformed/trailing JSON, bad type/UTF-8 and depth33. Full1048576-byte controls and depth32 pass. Required checks pass; production coverage12296/13032 =94.35%.

Step8 completed: typed known discriminators and required/optional representation validation cover metadata, quotes, results, failures and Ledger controls, with canonical identities, exact fixed-byte sizes, Base64 and unsigned fields. Deleting/nulling every required field fails; semantic authority remains at exchange ports. Required checks pass; production coverage12532/13279 =94.37%.

Step9 completed: exact integer widths, canonical64-bit strings and lowercase IEEE wrappers preserve pinned values and signed zero; exceptional floats require trusted contract permission and canonical NaNs. Independent extrema/invalid-form tests and required checks pass; production coverage12608/13354 =94.41%.

Step10 completed: canonical inline bytes enforce the4096 decoded-byte threshold, exact wrappers and empty-inline representation independently of control identifiers. Fixed examples, padding/unused-bit rejection and4097-byte refusal pass. Required checks pass; production coverage12623/13372 =94.40%.

Step11 completed: schema-directed provisional metadata validates deterministic UTF-8/index traversal, unique sequential IDs, exact descriptors and aggregate declared budgets before body allocation. Complete assembly preserves typed known values and consumes ignored extension bodies without application authority. Required checks pass; 12842/13606 = 94.38%.

Step12 completed: ordered receivers use public Services Body buffers, validate exact ID/offset/chunk bounds and final UTF-8, poison malformed transfers and expose only a consumed complete Value. Split multibyte text, missing/extra data, wrong IDs/order and inline-only cases pass. Required checks pass; production coverage12924/13683 =94.45%.

Step13 completed: immutable sources validate exact IDs/lengths/text and aggregate bounds; queue-aware chunks preserve offsets under backpressure and never exceed65536 data bytes. Tiny-queue and maximum-chunk tests pass. Required checks pass; production coverage12988/13751 =94.45%.

Step14 completed: direction-aware metadata, invocation, wanted and Ledger ordering requires correct markers/ENDs, rejects early bodies/results and preserves accepted uncertainty after interruption. Independent exchanges remain usable after a poisoned instance. Required checks pass; production coverage13141/13907 =94.49%.

Step15 completed: real Board Read contexts serve exact current Board/definition controls under shared response bounds; absent offerings return not_available without synthetic definitions or Ledger/Services execution. Directional END and authority tests pass. An existing stale temporary identity-path collision was corrected separately. Required checks pass; production coverage13173/13941 =94.49%.

Step16 completed: opaque local metadata requests require separate outgoing/incoming authority, exact original endpoint/connection/stream association and captured deadlines. Raw subdocument spans feed public Board cache store/attach; wrong attribution, forwarded receipts and stale definitions fail. Required checks pass; production coverage13305/14090 =94.43%.

Step17 completed: provided proposals bind original peer/stream, selected publisher/issuer/contract, installed Registry and read-only Ledger trust/credit policy before callbacks or holds. Added an explicit Services unresolved-input metadata contract and conservative assessment defaults instead of placeholder bodies. Actual-module refusal and large-body metadata tests pass; required checks pass at13671/14533 =94.07% production coverage.

Step18 completed: exact current provider quotes retain random IDs, raw JSON/hash, permitted factors and original payment fields under finite expiry/capacity. Scoped quote generations prevent stale handles from accepting or releasing replacements; failed randomness or inconsistent family pricing leaves no hold/work. Required checks pass; production coverage13921/14822 =93.92%.

Step19 completed: requester-only installed codecs validate original-stream quote attribution, exact retrieved contract, issuer/payment equality, specified factors and independently calculated prices. Supported term changes require separate exact-hash local consent under both spending ceilings. Raw whitespace is retained for hashing; invalid contracts, factors, fees, authority and streams cause no holds/execution. Required checks pass; production coverage14270/15188 =93.96%.

Step20 completed: exact quote ID/hash and authenticated stream validation precede provider admission; requester consent reserves actual Ledger capacity and journals one dispatch before exposing accept bytes. Two independently owned stacks verify holds, unfunded/foreign acceptance, duplicate dispatch refusal and retained obligations after send/session loss. Rejected exchanges release only transient quotes. Required checks pass; production coverage14446/15373 =93.97%.

Step21 completed: reserved input is gated on exact bodies, input.end and original requester END before complete family validation and once-only effectful dispatch. Ordered senders retain original sources under bounded queue backpressure; malformed/early/duplicate controls and incomplete transfers cannot dispatch. Required checks pass; production coverage14559/15492 =93.98%.

Step22 completed: a new public committed-invocation observation port supplies original terms and retained output; senders cannot emit uncommitted results. Output price/finalPrice must equal the independently validated envelope during Services retention and wire transmission. Metadata/body/result.end queues preserve progress and durable ownership on backpressure/loss. Required checks pass; production coverage14659/15589 =94.03%.

Step23 completed: requester output stays inaccessible until exact bodies, result.end and normal original-stream completion, then passes independent Services content/schema/final-price validation. Actual two-sided commitment tests expose output only via the committed result port; wrong peers/content/prices, missing markers and abnormal completion retain accepted obligations. Required checks pass; production coverage14777/15712 =94.05%.

Step24 completed: all specified failure codes remain distinct, unknown codes are generic, diagnostics are bounded and access can be concealed. Actual provider abort acknowledgement is required before claiming an accepted failure; requester holds release only after a complete normal failure exchange. Failures after result.end, committed response loss and interrupted END cannot manufacture rollback. Required checks pass; production coverage14895/15833 =94.08%.

Step25 completed: installed prospective providers create owner-scoped wanted invitations, and publisher-side local policy alone supplies input/spending after exact invitation/current-term checks. Real Registry/Board/Services/Ledger tests complete reversed-role work and settlement, reject absent providers and refused/malformed invitations, and keep untrusted input/installation suggestions inert. Required checks pass; production coverage15040/15988 =94.07%.

Step26 completed: challenge-only paid checks bind exact quotes, immutable mode/price/response terms, two-sided reservations, terminal gates and trusted timing beginning at input.end. Evidence schemas exclude provider outcomes; exact standard identifier/sample widths and actual custom-resource checks verify paid wrong evidence, private expectations, malformed/late/missing evidence, local-only refusal and entropy failure. Unresolved pre-quote ownership keys are explicitly unsupported; included consumption remains step27. Issue10 still limits global response-loss charging claims. Required checks pass; production coverage15876/16920 =93.83%.

Step27 completed: included allowances now distinguish in-flight reservation, complete consumption, definitive incomplete/acknowledged-abort release and uncertain guards. Complete requester evidence consumes only after normal terminal reception; provider commitment alone does not. Wire-owner delivery observations consume sender capabilities, and concurrency/restart/lost-write tests retain original scope limits and conservative legacy guards without new receipts. Paid issue10 remains separate. Required checks pass; production coverage16005/17044 =93.90%.

Step28 completed: explicit family-approved Messaging recovery/removal retries match original caller, placement/ID and every defined request/control field before current publication, prices or balances. Opaque cached/explicit-retry capabilities permit the narrow early-result ordering exception and original-key result application without new work, holds or charges. Pinned full-schema narrow fixtures verify detached cached output, drain/removal, depleted funds, conflicting fields, unrelated callers and unknown outcomes; they do not implement Messaging signatures/resources. Upstream recheck on2026-11-14 found no protocol changes. Required checks pass; production coverage16443/17477 =94.08%.

Step29 completed: caller-scoped balance and protected original/new-owner transfer queries use real Ledger Control ports without Services work or book mutations. Complete requester observations bind original issuer/connection/stream, tuple and exact gross/fee/net under normal termination; unrelated existing/missing queries are concealed. Strict real Ed25519 transfer fixtures exercise the protected records. Required checks pass; production coverage16633/17678 =94.09%.

Step30 completed: real signed transfers quote trusted exact fees without reservation and commit only after original-stream ID/fee acceptance and atomic capacity recheck. Explicit client consent distinguishes fresh accepted fees from stored historical replies; signed retries bypass changed fee policy and cannot recharge. Zero fees, invalid proofs/IDs/streams, expiry, depleted funds and conservative interruption pass. Required checks pass; production coverage16949/18012 =94.10%.

Step31 completed: exact bounded encrypted claim controls use Ledger authenticated decryption/owner replay ports and return only original public ID/stored amount. Real generic encryption verifies complete authentication, wrong-owner/tamper refusal, predecryption size checks, unavailable keys, once-only issuance and normal requester completion. Claim interruption remains Unknown; no administrator wire operation is exposed. Required checks pass; production coverage17058/18129 =94.09%.

Step32 completed: original committed third-party payment actions bind direct issuer queries to independent connections and exact retained owner/newOwner/ID/gross/fee/net. Opaque normal-completion observations plus separate Evidence/Coordinate grants settle each local book once; forwarded requester data, changed fees and reused bindings fail without reversing work. Real signed issuer transfers and three financial owners verify unknown/unsettled states and idempotent confirmation. Process/network service evidence remains steps33–35. Required checks pass; production coverage17149/18229 =94.08%.
