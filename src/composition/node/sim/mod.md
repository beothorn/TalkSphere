# Policy simulation

A deterministic harness for comparing peer-policy parameters before real usage can. It is test and research tooling, never
production networking, and it is Linux-only like the node composition.

| Contract | Responsibility |
| --- | --- |
| [Rng](rng.md) | Seed-determined random numbers (SplitMix64) |
| [Scenario](scenario.md) | The simulated population, prices and the policy under test |
| [World](world.md) | The simulation loop and its measurements |
| [Abuse](abuse.md) | Request-rate abuse against the quota policy |

**What is real:** the policy contracts (`Configured`), the observation table, the replica book (volatile backing), the
maintenance planner and the CR2SE redundancy report, exactly the code the node runs. **What is modelled:** time (a controlled
clock), the network (a client is connected or not), Storage (a copy exists until its holder is wiped) and prices. The wire
protocol is not exercised; the process tests cover it. Every figure is therefore a function of the model's assumptions, which
real usage must later replace.

A run is reproducible from its seed and produces a machine-readable report. Run it with
`cargo run --locked --example policy_sim -- help`.
