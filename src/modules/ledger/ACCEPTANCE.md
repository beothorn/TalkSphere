# Ledger acceptance evidence

Behavior and fixtures use CR2SE revision
[d40b6278f24430e814d7fa82094f57261bfcac9f](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f).
Rechecked 2026-11-08: six later website/workflow/introductory commits change none of
these contracts. Sources and exact requirements are in [REQUIREMENTS.md](REQUIREMENTS.md).

## Claimed scope and binding

Direct-call Ledger participant, exact local accounting, all issuer relationships,
protected Linux persistence, canonical signed transfers, trusted authenticated issuer
evidence, generic encrypted claims and bounded offline inspection. Cryptography and
Identity adapters are real; scripts substitute only controlled entropy/failures,
coordinator/handler decisions and authenticated issuer exchange responses. No live
Peer Protocol ledger exchange, serialized module IPC, actual Services coordinator,
minimum useful node, cross-peer exactly-once or physical power-loss claim is made.

Run from repository root:

```sh
cargo test --locked modules::ledger
cargo test --locked --test ledger_persistence --test ledger_cli
```

## Assertion map

All named tests execute under the required full suite. Logical rows below identify
what is established; the last column preserves later composition obligations.

| Upstream case | Passing local assertions / test names | Boundary |
|---|---|---|
| LG-01 | `issuer_views_are_exact_and_scoped`: owned100/40, max issued, no self/aggregate view; `atomic_transfer_and_replay`: caller-only balances, protected queries | Actual network balance exchange later |
| LG-02 | `literal_two_party_and_internal`: price7, hold leaves100/available93, requester/provider independently reach93 and duplicate commit is inert | Local peers, no cross-peer atomicity |
| LG-03 | `literal_two_party_and_internal`, `requester_capacity`, `internal_budget`: issuance10/price7 -> issued/owned7, remaining3, no self-balances; finite second hold refused | Actual wanted-service dispatch later |
| LG-04 | `concurrent_spending`, `local_commit_modes`, `abort_decisions`: one60 hold from100, settle21 ->79, unused39 released, explicit failure uncharged | Barrier-driven local serialization |
| LG-05 | `exact_maximum_and_authority`, `incoming_exact_maximum`, `forged_contexts_and_tokens`, `transfer_accounting_refusals`: exact max->0/MAX, invalid values, zero/bounds/underflow/overflow, forged holds/effects and unauthorized admin rejected | Negative/overwide values tested at exact local decoder; Peer JSON parser later |
| LG-06 | `explicit_bootstrap`, `encrypted_owner_binding`: balance/auth alone gives no trust, explicit grant5 only, bounded zero-trust encrypted claim redemption, no automatic trust increase | Board metadata remains Board's bounded uncharged control |
| LG-07 | `literal_order_and_refusals`, `real_transfer_signatures`, `signed_transfer_journey`: exact121 offsets/signing bytes, amount9, exact fee2, issuer source91/recipient7/net7 | Real cryptography, local fee-control ports |
| LG-08 | `literal_order_and_refusals`, `real_transfer_signatures`, `exact_stream_and_fee`, `transfer_accounting_refusals`: legacy/truncated/version/signature/key/caller/issuer/amount/recipient/fee/headroom/source refusals; quote alone keeps balance100 | Stream identity is trusted adapter context; network message-order parser later |
| LG-09 | `atomic_transfer_and_replay`, `signed_transfer_journey`: exact original result/fee survives restart and maintenance, changed signed order conflicts, only old/new owner queries succeed | Lifetime replay cannot be evicted at capacity |
| LG-10 | `issuer_payment_evidence`, `third_party_binding`, role refusal tests: exact original issuer evidence settles each role once, wrong fields/forwarded-control authority/reused tuple rejected, unknown/refusal retained separately, explicit action requests never execute work | Real network original-issuer query deferred |
| LG-11 | `finite_creation`, `portable_literal`, `encrypted_owner_binding`: independent random32/32, exact105 layout, literal commitment oracle,7 reserved from20, real encryption and owner binding, issued7 once, no trust or clear bearer wire/log exposure | Actual peer envelope carriage later |
| LG-12 | `portable_literal`, `finite_creation`, `encrypted_owner_binding`: malformed/noncanonical/zero/secret/recipient/owner/invalidation rejected; same owner replay after restart returns stored7 despite portable99; invalidation race preserves one terminal winner | Private portable transfer is caller-owned confidential delivery |
| LG-13 | `prepared_status_and_conflicts`, `abort_decisions`, `coordinator_fault_matrix`, `unknown_gate_and_recovery`: exact preparation/token/status, commit/abort conflict, no timeout rollback, recovered decision delivery once | SV-09 uses real Ledger with test coordinator/handler; real Services integration remains deferred |
| AT-C01 | Authority tests, `forged_contexts_and_tokens`, `protected_signing_and_decryption`, `error_categories`: domain/purpose/identity/version/byte checks; real protected key adapters; no peer pointer/path fields | Direct-call only; no serialized/process-separated module adapter claim |
| AT-C02 | Limits/framing tests, `commit_cut_matrix`, `unknown_gate_and_recovery`, `retained_obligations_and_limits`: finite admission, bounded load, explicit dependency failure, unknown commits gate and reload, no automatic new work | Linux cuts simulate failure, not physical power loss |
| AT-C03 | Complete-image copy/reopen, prepared retry, lifetime transfer/claim tests and reduced-limit maintenance: copied values and accepted terms survive, wrong owners denied, obligations/replay retained | Transient grants are not durable authority |
| AT-C04 | `ledger_trace_excludes_payloads_and_claims`, encrypted tamper tests, cancelled-call tests and CLI scenarios: secret-safe bounded diagnostics, complete authenticated results only, no hidden network/grants/refunds | Family check evidence belongs to Services/family modules |

## Recovery and persistence evidence

`real_store_and_protection`, `unsafe_metadata` and `commit_cut_matrix` test no-follow
paths, ownership/modes/link policy, writer locks, malformed state and every staged
write/sync/rename cut. `committed_state_survives_process_reopen` uses a separately
executed process with no parent heap/descriptors; `exclusive_writer_across_processes`
checks real competing-process refusal. Local unknown acknowledgement is reconciled
through complete-image reload, never by repeating a mutation or service execution.

Maintenance compacts only zero balance entries. All operation bindings are retained
until a future explicit coordinator retention proof exists; lifetime transfer/terminal
claim records are never erased. Finite admission can therefore refuse new work.
Smaller reopen limits and unsupported private schema versions fail without repair.

## Fixtures, policy and limits

Layout and arithmetic expectations are literal independently derived examples, not
claimed standalone upstream Ledger vectors. Existing pinned public crypto seeds are
reused for actual Ed25519/envelope behavior. Test-only scripted dependencies stay
under excluded test support. Exact trust is local integer millionths, default0;
requester-credit and third-party risk acceptance are explicit provider policy.
Unconfirmed third-party holds remain conservative until authenticated resolution.
Local storage formats and grant tokens add no wire fields.

No new confirmed language-independent protocol defect was identified in this module;
implementation recovery/validation defects were corrected locally. Existing Board
issues do not change Ledger behavior. Final required checks pass; production line coverage is **8791/9150 = 96.08%**
under [the production procedure](../../../docs/coverage.md). Formatting, strict
Clippy, tests, build, overview/detailed help, warning-free API docs and the runnable
accounting example pass. Passing round trips alone does not establish
interoperability or security under every failure.
