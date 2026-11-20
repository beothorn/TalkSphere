# Requester native recovery evidence

The requester tests reopen separate Linux Services and Ledger FileStores using
only their protected, synchronized files. Every reopen constructs fresh module
heaps, authorities, grants, Registry and installed Storage requester codecs;
initial funding is performed only when the files are first created. Recovery
never initializes replacement balances. All calls use public module contracts.
The test harness's persistence type parameters are test-only composition helpers.

The requirements remain pinned to
[CR2SE d40b6278](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f),
including [AT-E04](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/AcceptanceTests.md),
[AT-E06](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/AcceptanceTests.md)
and [durable coordination](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/ModuleContracts.md#durable-effects-across-process-boundaries).
Upstream main and issue #11 were rechecked on 2026-11-21; reviewed protocol files
are unchanged and the acceptance-time conflict remains unresolved.

| Scenario | Executed assertions |
| --- | --- |
| Store, retrieve, renew and remove, each reopened after staging, preparation, commit decision and final acknowledgement | Original phase and hold survive; fresh-authority resume commits once; stale authority is denied; every typed output field and exact price survives. |
| Second reopen after completion | Retained result/effect bytes match exactly; the original provider-currency balance and available amount remain charged once. |
| Requester-issued credits at each completion boundary | Finite issuance survives reopen; the provider has exactly one issued balance after resume; no self-balance is fabricated. |
| Third-party Pending, Unknown and Unsettled payment, each independently reopened | Committed output, exact gross/fee/transfer binding and original owned-credit hold survive; work cannot be resent. |
| Independent issuer commits a real signed transfer and its reply is discarded | After native issuer reopen, original/new owners can query the exact original result; unrelated callers cannot; gross debit and net credit are exact. |
| Settlement followed by requester reopen | Exact evidence settles once, stale local evidence authority is refused, fresh original evidence is idempotent, and retained Storage output/effect bytes and local trust are preserved. |
| No remote result or output missing result.end | Repeated reopen retains Executing and its original hold; resume stays Unknown; outbound cannot resend; no successful result is exposed. |
| Local abort attempted after dispatch, staging or preparation | Both before and after reopen, abort cannot erase potentially committed remote work, release the hold or manufacture success. |
| Ledger preparation durable, reply lost | Services remains Staged; reopen recognizes original preparation and finishes once. |
| Services preparation durable, reply lost | Reopen observes Prepared and finishes the original decision sequence once. |
| Commit decision durable, reply lost | Reopen observes Commit and delivers that decision without replacing it. |
| Ledger charge durable, reply lost | Reopen observes pending coordinator commitment with the original already-applied charge; acknowledgement does not charge again. |
| Final Services acknowledgement durable, reply lost | The uncertain live owner exposes no success; reopen observes Committed with exact retained output and one charge. |
| Ledger commit, Services preparation or Services decision refused before writing | Reopen sees the last acknowledged phase and original hold; authorized resume reconciles participant progress without a duplicate charge. |

Fault wrappers delegate to real native adapters. A before-write refusal performs
no write; a lost reply is injected only after the adapter has acknowledged file
and directory synchronization. Coordinator and participant controls are independent
and discarded at reopen; they are not runtime configuration or protocol behavior.

These are local drop/reopen and injected I/O-reply tests. They are not subprocess
kill, machine power-loss, authenticated node-process or independent implementation
interoperability tests. Remote quotes/results are controlled fixtures; no Storage
provider runs and no paid lease is created. Original private lease expectations
are explicit fixtures, not a demonstrated durable requester catalog. The independent
issuer in the added native payment tests also reopens its own protected Ledger
file; the earlier focused settlement tests use Memory. Both groups supply trusted
synthetic issuer provenance rather than an actual issuer Transport connection.
These tests do not complete AT-E06's provider-side direct query/settlement path.

This advances requester recovery in steps 29, 31 and 32. Full step 32 additionally
requires real provider bytes/metadata, all Storage participant cuts, actual provider
obligations and remote uncertainty, pending step 17 and
[CR2SE issue #11](https://github.com/beothorn/CR2SE/issues/11). None of these plan steps is
marked complete by this evidence. The tests do not introduce remote cached retries,
refunds, replacement stores or a runnable paid Storage provider.

From the repository root on Linux:

```sh
cargo build --locked
cargo test --locked --lib modules::storage::requester::client::tests::wire::recovery
cargo run --locked -- --help
```

Each scenario creates private temporary directories, closes both owners before
reopening and removes only its own directory afterward. No production state,
network socket, operator configuration or peer message is created by these tests.
