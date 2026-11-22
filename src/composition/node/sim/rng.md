# Simulation random numbers

`Rng::new(seed)` is SplitMix64: one 64-bit state fully determined by the seed, identical on every platform. `next_u64`, `unit`
(uniform in `[0, 1)`), `below(n)`, `chance(p)` and `exponential(mean)` (whole seconds, at least one). It is for simulation only and
must never be used for keys, identifiers or any security decision.
