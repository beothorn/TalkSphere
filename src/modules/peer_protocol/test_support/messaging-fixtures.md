# Narrow Messaging replay fixtures

messaging-definition.json is the literal complete definition in section16 of
[Messaging at d40b627](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/messaging/README.md#16-required-service-definition).
The test-only catalogue compares the full pinned document before installation.
Offering economics use its complete model/info/precondition shape, with minimum7,
zero rational factors, removal7 and zero release credits.

messaging.rs is a narrow injected replay port, not a production Messaging backend.
Its signatures and identities are fixed unverified fixture data. It does not claim
store/retention, receipt/signature verification, encryption, discovery or complete
Messaging conformance. Its purpose is to exercise Peer Protocol/Services cached
binding with the pinned logical field shapes, real independent Ledger/Services
owners, detached payloads and once-only test participant decisions. The fixture
installs nothing in serve or normal application composition.

From the repository root use `cargo test --locked --lib replay_tests`,
`cargo build --locked` and `cargo run --locked -- --help`. Test results demonstrate
local binding/accounting, not cross-implementation or authenticated network evidence.
