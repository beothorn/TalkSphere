# Storage and minimum-node handoff

Paid Storage is complete for a finite native Linux loopback profile. See the
[plan](../../../plans/9_Storage_module/PLAN.md) and
[acceptance evidence](ACCEPTANCE.md). Behavior follows
[CR2SE 24cce05](https://github.com/beothorn/CR2SE/tree/24cce05bb258211b1fe07914bcd0a7743cfcd91c)
and its later fix [bc53df4](https://github.com/beothorn/CR2SE/commit/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16).
Upstream issues [#10](https://github.com/beothorn/CR2SE/issues/10),
[#11](https://github.com/beothorn/CR2SE/issues/11) and
[#12](https://github.com/beothorn/CR2SE/issues/12) are resolved; see
[CR2SE bugs](../../../CR2SE%20bugs/README.md).

## Supported profile and limits

- Linux with comparable uptime inside one boot; protected native Board, Services, Ledger, Identity and Storage owners; authenticated Transport.
- One lease is one copy at one provider. Replication, encryption and catalogs are requester work.
- Limits are finite: capacity bytes, lease slots, pending operations and retained results are configured by the composing owner. Exhaustion refuses before work.
- Production line coverage measured 93.56% with the [coverage procedure](../../../docs/coverage.md).

## Startup recovery

Open the Storage owner and recover participants before admitting new work, then
open Board, Services and Ledger with the same identity. Prepared effects resolve
from their durable Services decisions; physical cleanup resumes in bounded
batches and never reactivates an expired lease or discards a live pin.

## Composition for the minimum node

- Install the standard definition and provider handler through Services and publish through Board only after recovery succeeds.
- Bind requester operations to Peer Protocol with explicit consent and total spending bounds.
- Default `serve` does not install the paid Storage handler; the node milestone must add that composition and operator configuration.
- Ledger credit policy (provider-issued, requester-issued, third-party) stays an operator choice; unsupported policies are refused.

The minimum useful node and full client journey remain open roadmap milestones.
