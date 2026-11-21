# Minimum useful node acceptance inventory

Behavior baseline: [CR2SE bc53df4](https://github.com/beothorn/CR2SE/tree/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16),
rechecked 2026-11-21. It adds nothing newer than the Storage fixes already adopted
(issues [#10](https://github.com/beothorn/CR2SE/issues/10),
[#11](https://github.com/beothorn/CR2SE/issues/11) and
[#12](https://github.com/beothorn/CR2SE/issues/12)). No new protocol defect is asserted.

Sources: [minimum useful profile](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/PoC.md#minimum-useful-profile)
and [AT-E01–AT-E08](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/AcceptanceTests.md#end-to-end-acceptance).

## Gap inventory (step 1)

| Requirement | Existing module evidence | What only a composed node can prove |
| --- | --- | --- |
| Authenticate a configured contact and fetch Board and definition | Peer Protocol and Storage examples in one test process | Two `serve` processes, each with durable owners |
| Reserve finite credits, store a nonempty object, retain the lease | Storage exercise with demo owners and an in-memory catalog | Operator-configured offering, durable requester catalog |
| Restart the provider and retrieve with a new paid request | Storage process fixture | Restart of the production composition, recovery before admission |
| Renewal, byte check, removal, unauthorized access, stale expiry | Storage tests and exercises | Operator commands against a running node |
| Concurrent reservations, uncertain output delivery | Services, Ledger and Storage tests | Concurrent control requests on one node |
| Optional families absent and never advertised (AT-E01, AT-E05) | Services refusal tests | Board contents and refusal over a real connection |
| Authorized local control | Library calls only | Owner-only control channel on a running node |
| `serve` installs Peer Protocol and paid Storage | `serve` registers no application protocol | The node runtime |

Matching rows for the executed tests are added at step 24.
