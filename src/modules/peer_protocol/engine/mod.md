# Peer Protocol engine primitives

The engine boundary connects authenticated Transport observations to bounded
asynchronous module calls. Its [worker pool](worker.md) is available; the opt-in
[authenticated stream adapter](bridge/mod.md) is available. Complete production
handler composition is still under development. The default server installs neither
a factory nor this adapter. Explicit registration requires a trusted factory;
these primitives alone do not establish protocol conformance.
