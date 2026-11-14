# Aggregate endpoint admission

`Budgets` configures finite transient logical `value_bytes`, detached `bodies`
and active handlers `per_peer`. Defaults are268435456 bytes,65536 references and
four handlers per authenticated remote identity across all connections. These
limits apply across incoming/outgoing factories and all endpoint clones. Invalid
or insufficient startup budgets fail explicitly. The wire's own value bounds
and each durable module owner's journal/body limits remain independent.

Each admitted handler reserves four times its configured maximum value bytes for
two directions and transient assembly/encoding copies. Its body-reference
reservation is twice the lesser of the per-value reference limit and the maximum
number of detached4097-byte bodies that fit that value budget, with a minimum
reservation of one. Aggregate byte/reference quotients and the configured
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
