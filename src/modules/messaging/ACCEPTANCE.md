# Messaging acceptance evidence

Status: complete for the planned module scope (plan steps 1-38): the engine, the Services provider adapter and requester client, the
Peer Protocol requester intents, crash recovery across independent Services, Ledger and Messaging files, Identity-backed payload
decryption and the authenticated four-node exercise with replicas. Nothing is advertised on a Board by default. See the
[plan](../../../plans/11_Messaging_module/PLAN.md) and [HANDOFF.md](HANDOFF.md).

Behavior uses [CR2SE 73edf2d](https://github.com/beothorn/CR2SE/tree/73edf2dc230bccb0f27f2ea6e06e938fff738862); byte-exact
vectors were generated independently with PyNaCl (`fixtures/generate.py`), not by TalkSphere.

| Case | Executed evidence (all in `cargo test --locked --lib modules::messaging`) | Remaining acceptance scope |
| --- | --- | --- |
| MS-01 | `signing::ms_01_*` match the independent vector and reject every mutation. | |
| MS-02 | `provider::ms_02_*`: durable pending placement, `acceptedAt`/`expiresAt`, price 17, a second store is a new placement. Native store reopen in `store_tests`; `service::crash` reopens independent Services, Ledger and Messaging files at every boundary. | |
| MS-03 | `provider::ms_03_*` and `pricing`/`terms` tests: every rejection leaves nothing charged, reserved or recorded; prices round once. | |
| MS-04 | `provider::unauthorized_and_unknown_placements_are_indistinguishable`, `pending_limits_bound_each_sender_and_recipient`. | |
| MS-05 | `provider::ms_05_*`, `discovery_*`: order, exclusive cursor, `more`, empty page charged. | |
| MS-06 | `receipt` tests match the independent request/receipt vectors. | |
| MS-07 | `provider::ms_07_*`: original message, receipt verifies, discovery excludes it. | |
| MS-08 | `provider::ms_08_*`; `service::tests::a_recovery_retry_returns_the_original_output` (Services cached provider) and the endpoint test's retry over a new stream, all at no new charge. | |
| MS-09 | `provider::ms_09_*`: sender-only status with the exact recorded receipt. | |
| MS-10 | `provider::ms_10_*`: guarded renewal, one winner among eight threads. | |
| MS-11 | `provider::ms_11_*`, `quote_removal_*`, ledger `release` tests; `service::tests::removal_issues_release_credits_through_the_ledger`; `service::crash` (credits issued once even with a lost Ledger acknowledgement). | |
| MS-12 | `provider::ms_12_*`, participant removal tests: retry returns the recorded result after expiry and restart. | |
| MS-13 | `provider::ms_13_*`, `check` tests and the endpoint test `a_paid_check_is_judged_with_the_senders_private_expectation` (real endpoints, exact charge, pass/fail/inconclusive). | |
| MS-14 | `encryption::ms_14_*` reproduces the independent payload byte for byte. | |
| MS-15 | `credential` tests validate the Board `identityEncryption` credential. | Publication by a node (client journey step 3). Identity's agreement operation exists (`identity::operations::agree`, `messaging::identity`). |
| MS-16 | `lifecycle` and `provider::ms_16_*`: recover/remove, renew/remove, exact expiry boundary, status observation. | Concurrent races are tested on the engine; the coordinator serializes per operation, and `service::crash` covers decisions at every boundary. |
| MS-17 | `receive::replicas_of_one_message_are_delivered_once` and the validation-failure test; `cargo test --locked --example messaging` recovers one message from two real authenticated providers, decrypts it with the recipient's protected identity and delivers it once. | |

## Added evidence

- `cargo test --locked --example messaging` (and `cargo run --locked --example messaging -- run`) runs a sender, a recipient and two
  providers with protected identities over authenticated loopback TCP and native stores: replicas at both providers at exact prices,
  discovery, a signed recovery, decryption through the recipient's protected identity, one delivery of two replicas, status with
  receipt, renewal, a paid byte check and a removal releasing credits once. `cargo test --locked --test cli messaging` checks the help topic.

- `service::tests` runs every operation through the real Services coordinator, Board and Ledger with exact charges and decision-time
  fields; `peer_protocol::engine::endpoint::tests::messaging` runs the real family over three independent module stacks and real
  endpoints (every operation at its exact price, a retry over a new stream, a paid check, a quote above the requester's own price refused).
- `service::crash` covers a store, a recovery and a removal at every coordination boundary with exact balances, placements, receipts
  and release credits; it exposed and fixed stateless template validation.

- A race found while testing renewals (a second identical renewal accepted as a replay of the first) was fixed in
  `Participant::prepare`, and the test passes 200 of 200 repeated runs.
- The Ledger gained `issue_release` (once-only issuance keyed by operation) with its own tests in `modules/ledger`.

## Known limits

- Removal tombstones are kept for the ledger's lifetime and count toward the placement bound.
- The recipient's dedupe table is in memory; the persistent client catalog belongs to the client journey.
- Incomplete uploads are held in memory under a bounded reservation and are not durable.
- Nothing is advertised on a Board by default; a node installs the handler and publishes the offering through its composition.
- The authenticated exercise runs four nodes in one process (no provider restart or separate provider process); see the guide.

## Checks and coverage

`cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo build --locked` and `cargo run --locked -- --help`
pass, and the instrumented full test run (every unit, integration and process suite) exits 0. Production line coverage, measured with the
[coverage procedure](../../../docs/coverage.md) with the exclusion regex extended to `/test_support\.rs`, `_tests\.rs` and `/crash\.rs`
(Messaging keeps its test files beside the source), is **34358/36989 lines (92.89%) for the repository and 4038/4283 (94.28%) for
`src/modules/messaging`**.
