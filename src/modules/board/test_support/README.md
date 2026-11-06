# Board deterministic test support

Compiled only under cfg(test) and excluded from production coverage. Store scripts
three cut points: before commit, committed with lost acknowledgement, and committed
with acknowledgement. Restart keeps only durable bytes and resets volatile scripts
and counters. Script exhaustion is failure, never implicit success. It models
atomic commit outcomes; it does not prove any filesystem implementation durable.

Clock advances controlled seconds with checked arithmetic. Capabilities declares
exact versions, required semantics and literal pricing outcomes, and performs no
network or service execution. Real local grant tests reuse Board Authority.

Document strings are derived from the pinned CR2SE Board/Services specifications
in the plan. The issuer comes from the pinned public Transport vector through
Identity's strict text codec; it is not an authenticated retrieval context. Main,
large-exact and maximum prices are independent literal expectations. These are
not published upstream Board vectors. Definition fixtures contain all identity,
input/output and check fields and never use the production Board encoder.

Concurrent publication tests should use standard Barrier/channel controls at
actual publication boundaries; no scheduler facade or sleeps are required. Fresh
process tests can reuse tests/support/runtime.rs once durable wiring exists.

From the repository root: cargo test --locked modules::board::test_support.
