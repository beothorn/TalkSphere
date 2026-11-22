# Messaging acceptance evidence (interim)

Status: **interim**. The engine below (validation, placements, the durable participant, every operation, receipts, encryption,
requester checks and message acceptance) is implemented and tested. The Services provider adapter, the Peer Protocol requester,
authenticated multi-process exercises and crash recovery across Services and Ledger files are **not** implemented, so the
module is not complete and is not advertised as a supported capability. See the [plan](../../../plans/11_Messaging_module/PLAN.md).

Behavior uses [CR2SE 73edf2d](https://github.com/beothorn/CR2SE/tree/73edf2dc230bccb0f27f2ea6e06e938fff738862); byte-exact
vectors were generated independently with PyNaCl (`fixtures/generate.py`), not by TalkSphere.

| Case | Executed evidence (all in `cargo test --locked --lib modules::messaging`) | Remaining acceptance scope |
| --- | --- | --- |
| MS-01 | `signing::ms_01_*` match the independent vector and reject every mutation. | |
| MS-02 | `provider::ms_02_*`: durable pending placement, `acceptedAt`/`expiresAt`, price 17, a second store is a new placement. Native store reopen in `store_tests`. | Restart through the Services coordinator (step 33). |
| MS-03 | `provider::ms_03_*` and `pricing`/`terms` tests: every rejection leaves nothing charged, reserved or recorded; prices round once. | |
| MS-04 | `provider::unauthorized_and_unknown_placements_are_indistinguishable`, `pending_limits_bound_each_sender_and_recipient`. | Peer-level reply bytes (step 31). |
| MS-05 | `provider::ms_05_*`, `discovery_*`: order, exclusive cursor, `more`, empty page charged. | |
| MS-06 | `receipt` tests match the independent request/receipt vectors. | |
| MS-07 | `provider::ms_07_*`: original message, receipt verifies, discovery excludes it. | |
| MS-08 | `provider::ms_08_*`: exact retry returns the recorded output and original price. | Services replay cache wiring (step 30). |
| MS-09 | `provider::ms_09_*`: sender-only status with the exact recorded receipt. | |
| MS-10 | `provider::ms_10_*`: guarded renewal, one winner among eight threads. | |
| MS-11 | `provider::ms_11_*`, `quote_removal_*`, ledger `release` tests: exact quote, credits issued once. | Charge and credits in one real Services decision (step 30, 33). |
| MS-12 | `provider::ms_12_*`, participant removal tests: retry returns the recorded result after expiry and restart. | |
| MS-13 | `provider::ms_13_*` (provider byte) and `check` tests (requester verdicts and timing). | Paid check over Services/Peer Protocol (steps 30, 31). |
| MS-14 | `encryption::ms_14_*` reproduces the independent payload byte for byte. | |
| MS-15 | `credential` tests validate the Board `identityEncryption` credential. | Publication by a node (client journey step 3). |
| MS-16 | `lifecycle` and `provider::ms_16_*`: recover/remove, renew/remove, exact expiry boundary, status observation. | Races across the Services coordinator (step 34 follow-up). |
| MS-17 | `receive::replicas_of_one_message_are_delivered_once` and the validation-failure test. | Replicas over real providers (step 35). |

## Added evidence

- A race found while testing renewals (a second identical renewal accepted as a replay of the first) was fixed in
  `Participant::prepare`, and the test passes 200 of 200 repeated runs.
- The Ledger gained `issue_release` (once-only issuance keyed by operation) with its own tests in `modules/ledger`.

## Known limits

- Removal tombstones are kept for the ledger's lifetime and count toward the placement bound.
- The recipient's dedupe table is in memory; the persistent client catalog belongs to the client journey.
- Incomplete uploads are held in memory under a bounded reservation and are not durable.
- No Messaging definition is advertised or registered with Services yet.
