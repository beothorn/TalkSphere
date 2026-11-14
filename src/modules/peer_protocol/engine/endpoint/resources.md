# Aggregate endpoint admission

`Budgets` configures finite transient logical `value_bytes`, detached `bodies`
and active handlers `per_peer`. Defaults are268435456 bytes,524288 references and
four handlers per authenticated remote identity across all connections. These
limits apply across incoming/outgoing factories and all endpoint clones. Invalid
or insufficient startup budgets fail explicitly. The wire's own value bounds
and each durable module owner's journal/body limits remain independent.

Each admitted handler reserves four times its configured maximum value bytes for
two directions and transient assembly/encoding copies. Its body-reference
reservation is twice the minimum of the per-value body-reference, logical-node
and logical-byte limits, with a minimum reservation of one. A detached body may
be just one byte; the inline encoder's4096-byte threshold cannot reduce received
reference accounting. Ignored extension bodies remain transfer obligations. Aggregate byte/reference quotients and the configured
exchange count determine the concurrent handler ceiling. This deliberately
conservative admission can reject an inline/control exchange despite its small
actual input. It measures bounded logical capacities, not allocator overhead,
arbitrary application callback memory or process RSS.

Admission is atomic before the factory constructs a handler or creates any
financial effects. Over-budget attempts fail with Limit; mutex failure refuses
new work. Accepted reservations stay attached through backpressure, dependency
waits, terminal callbacks and late cleanup until the worker-owned handler is
actually dropped. Constructor refusal returns its reservation. Cleanup releases
only transient capacity; it never aborts, evicts, retries or refunds durable
accepted work. One identity cannot evade its limit through extra connections.
Transport separately budgets framing, pending input/output and worker admission.
