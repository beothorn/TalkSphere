# Peer Protocol engine primitives

The engine boundary connects authenticated Transport observations to bounded
asynchronous module calls. Its [worker pool](worker.md) is available; the opt-in
[authenticated stream adapter](bridge/mod.md) is available. Complete production
handler composition is still under development. The default server installs neither
a factory nor this adapter. Explicit registration requires a trusted factory;
these primitives alone do not establish protocol conformance.

The [incoming endpoint factory](endpoint/mod.md) now composes provided work/check
and independent control responders over actual module owners. Complete outgoing,
wanted and production node factory wiring remain in progress.
