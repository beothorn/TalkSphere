# Parameter sweeps

Each sweep varies one family of parameters over seeds 1 to N, averages the measurements and returns `Table`s that render as
Markdown (`markdown`) or JSON (`to_json`). `Size` sets seeds, simulated days and items.

- `targets`: replica target 1 to 15 on the default flaky fleet.
- `fleets`: for a reliable, a flaky and a very flaky fleet, the smallest target that keeps data retrievable 99 % of the time, or the best tried.
- `check_intervals`: check interval 1 to 72 hours at target 5 with 5 % wipes a day.
- `dishonest`: dishonest share 0 to 50 % at target 5, with the credits they receive.
- `rate_limits`: `quota.requestsPerMinute` from none to 12 against flooding.
- `all`: every sweep, in that order.

Tables, their order and their numbers are reproducible. The results and what they suggest are in
[docs/policy/SIMULATIONS.md](../../../../docs/policy/SIMULATIONS.md); run `cargo run --locked --release --example policy_sim -- help sweep`.
