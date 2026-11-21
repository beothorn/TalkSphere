# Storage acceptance evidence

Behavior uses [CR2SE 24cce05](https://github.com/beothorn/CR2SE/tree/24cce05bb258211b1fe07914bcd0a7743cfcd91c).
Older literal fixtures retain their independent provenance. The Linux Storage
exercise authenticates TalkSphere nodes through real Transport and uses
independent protected Board, Services, Ledger, Storage and Identity owners.

| Case | Executed evidence | Remaining acceptance scope |
| --- | --- | --- |
| ST-01 | Pricing unit tests pin independent rational and overflow fixtures. | |
| ST-02 | Native Store and provider tests validate exact durable output, one charge, delayed selection and elapsed retention. The authenticated exercise reopens the original provider. | Full fault matrix under ST-12. |
| ST-03 | Store validation, byte-adapter faults, commit-time overflow and bad hash leave no active lease. | Native interrupted upload across process failure. |
| ST-04 | Owner-only lease unit tests and provider authorization collapse absent and foreign lookup. | Dedicated foreign authenticated process case. |
| ST-05 | Native range tests and authenticated detached retrieval verify complete bounds and whole-object hash. | |
| ST-06 | Accepted byte pins survive expiry and removal. A new operation beyond expiry is refused in owner tests. | |
| ST-07 | Guarded renewal tests verify concurrent expected expiry, checked duration and exact price. The authenticated exercise renews at the current provider. | Race coverage across shared endpoints. |
| ST-08 | Native removal tests and the authenticated exercise refuse later retrieval and preserve exact balances. | |
| ST-09 | Current-term and accepted snapshot unit tests exercise the Board and Services owners independently. | Storage-specific withdrawal after accept and current rate change at runtime. |
| ST-10 | Storage byte-check tests verify private expectations, matching/wrong bytes and missing evidence. Real authenticated paid check posts the exact amount on both sides. | Native provider commit with response loss. |
| ST-11 | Owner/offset refusal, optional overlap and interval bounds, trusted send timing in focused Storage and Peer Protocol tests. | Native interrupted evidence path. |
| ST-12 | Native requester recovery injects lost writes before/after original coordination and checks exact replay. Real provider restart preserves durable paid bytes. Distinct stores of identical ciphertext have separate IDs. | Full actual Storage provider fault matrix and native loss of successful response. |
| ST-13 / AT-E03 | Authenticated encrypted replica test backs up and restores the requester identity, restores catalog metadata, and decrypts verified bytes from the surviving provider. Two leases at the same identity still count as one provider. | |
| AT-E01 | Authenticated Linux example exercises detached store, retrieval, renewal, evidence and removal with exact balances. | Minimum useful node milestone. |
| AT-E02 | Authenticated wanted contribution issues exact requester currency and spends part back at its original issuer. | |
| AT-E04 / SV-09 | Native requester and Services decision tests recover original decisions and never rerun work. | Full actual Storage provider crash cuts and remote response loss. |
| AT-E06 | Native local requester and independent issuer tests retain exact payment across reopen. Peer Protocol verifies independent issuer confirmation for its test family. | Actual two-sided Storage provider settlement under third-party currency. |

Run `cargo test --locked --lib modules::storage` for bounded family and native
owner tests. Run `cargo test --locked --example storage` for the authenticated
network, independent provider process, encrypted recovery and wanted flows.
See the [exercise guide](../../../docs/storage/EXERCISE.md) and
[Storage plan](../../../plans/9_Storage_module/PLAN.md).
