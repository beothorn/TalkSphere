# Minimum useful node acceptance evidence

Behavior baseline: [CR2SE bc53df4](https://github.com/beothorn/CR2SE/tree/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16),
rechecked 2026-11-21. It adds nothing newer than the Storage fixes already adopted
(issues [#10](https://github.com/beothorn/CR2SE/issues/10),
[#11](https://github.com/beothorn/CR2SE/issues/11) and
[#12](https://github.com/beothorn/CR2SE/issues/12)). No new protocol defect is asserted.

Sources: [minimum useful profile](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/PoC.md#minimum-useful-profile)
and [AT-E01–AT-E08](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/AcceptanceTests.md#end-to-end-acceptance).

Profile: **minimum useful profile** only. The adapter is real TalkSphere at both ends: two `talksphere serve --node-root`
processes over authenticated loopback TCP, driven through the owner-only control socket. This is not
interoperability with another implementation, and it does not claim the full-feature PoC. Run it with
`cargo test --locked --test node_process`; see the [operator guide](RUNBOOK.md).

## Minimum profile demonstrations

| Demonstration | Executed evidence |
| --- | --- |
| Authenticate a configured contact; read the Board and definition | `at_e01_minimum_useful_node_between_two_processes` fetches over a pinned authenticated connection and requires exactly one provided offering, Storage. |
| Reserve finite credits, agree the price, store a nonempty object; retain lease ID, contact, hash, expiry | The same test stores `abc` for 92 credits; the catalog retains lease ID, provider, contact, hash, size, times and price (checked by `node-ctl leases` and the catalog unit tests). Encryption metadata is the application's: objects are stored exactly as supplied. |
| Restart the provider; retrieve and verify with a new paid request | The same test stops the provider with SIGTERM, restarts it on the same port from its durable owners, then retrieves `abc` and rechecks the whole-object hash. |
| Renewal, paid byte check and removal | The same test renews (+30 s), checks (pass) and removes. |
| Unauthorized access | `unauthorized_stale_and_expired_requests`: a third identity cannot see or use another requester's lease and pays nothing; identical content bought by it is a separate lease. Provider-side foreign denial is also covered by the Storage example `refuse_foreign`. |
| Stale expiration | The same test renews with a stale expected expiry; it is refused with no charge. |
| Concurrent reservations | `concurrent_stores_never_overspend_finite_credits`: two simultaneous stores against credits for one, exactly one succeeds, the other is refused as insufficient, and both owners agree on the total. |
| Uncertain output delivery | `lost_store_result_is_unknown_and_never_replaced` and `provider_crash_after_commit_applies_once` (see AT-E04). |
| Node API and every optional family absent; unsupported offerings not advertised or dispatched | `absent_optional_families_are_refused_without_charge`, and `only_storage_is_installed` (every optional family selects `Unsupported`). |

## AT-E01 assertions

| Assertion | Evidence |
| --- | --- |
| A and B with only the eight required modules, configured contacts, finite grants | The node composes Identity, Cryptography, Transport, Peer Protocol, Ledger, Board, Services and Storage only. Grants come from `node-ctl grant`/`own`/`trust`. |
| Authenticate B; read Board and definition; accept paid store of `abc` | `at_e01…` |
| Result hash is `ba7816bf…15ad` | Asserted exactly. |
| Save lease/contact/hash/expiry; restart B | The catalog persists across the provider restart. |
| Paid retrieve, renew, byte check and remove under current terms; bytes `abc`; check `pass` | Asserted. |
| One exact local charge per paid operation | Requester `owned` and provider `issued` both equal 100000 minus the sum of the five quoted prices, before and after the refused retrieval. |
| Removed lease unavailable; optional offerings absent | The retrieval after removal is refused with no charge; the Board lists only Storage. |

## AT-E04, AT-E05 and other end-to-end cases

| Case | Evidence and scope |
| --- | --- |
| AT-E04 lost remote result | The connection is cut after the provider commits but before the requester has the complete result: the requester reports `unknown_outcome`, journals it (it survives a requester restart), holds no lease, creates no replacement store, and the provider charged exactly once. |
| AT-E04 local crash | The provider is killed after committing; on restart it recovers with exactly one charge, serves again, and a new store is a separate explicitly authorized lease. |
| AT-E04 durable coordination cuts (SV-09) | Exercised at library level with real Storage and Services owners in `provider_reopens_every_coordination_boundary`; the process tests cover the commit boundary only, because a process cannot be cut at a chosen internal boundary without a test hook. |
| AT-E05 absent handlers | Not proven by a raw peer proposal (TalkSphere's requester only builds Storage requests); instead the provider registry selects `Unsupported` for each optional family before any acceptance, `node-ctl` has no such action, and the Board never lists them. The Files, Messaging and recovery parts are not implemented. |
| AT-E02, AT-E03, AT-E06 | Library and example evidence only (wanted Storage, encrypted replicas, third-party settlement); `node-ctl` exposes none of these. |
| AT-E07, AT-E08 | Not implemented: optional families and the Node API. |

## Added node-level evidence

- Shutdown during an upload and during a pending control request is prompt and clean, and restart is consistent (`shutdown_*` tests).
- Trace logs of all three nodes in a full flow contain no object content, no key bytes and no foreign lease id; standard output carries only lifecycle lines (`logs_and_replies_exclude_secrets_and_foreign_metadata`).
- Control: owner-only socket and root, bounded lines and connections, busy while stopping, exact integers, grant idempotency (library tests in `src/composition/node/control`).
- Initialization and inspection: `tests/node_cli.rs`.

## Reproduction

```sh
cargo test --locked --test node_process --test node_cli
cargo test --locked --lib composition::node
```

The proxy-based tests calibrate on a first run and so depend on identical traffic sizes between two fresh
node pairs; they passed in five consecutive full runs of `tests/node_process.rs` (nine tests each).

## Final checks

Formatting, Clippy with warnings denied, the full `cargo test --locked` suite, build, help output,
`RUSTDOCFLAGS='-D warnings' cargo doc` and `git diff --check` pass. Production line coverage under the
[documented procedure](../coverage.md) is 27453/29167 = 94.12%.
