# Cryptography

Status: implementation is progressing step by step; completed operations and
verification evidence are recorded in the [module plan](../../../plans/2_Cryptography_module/PLAN.md).
Primitives are exercised through local library calls; they do not start a server.
Identity persistence and authenticated server composition remain later work.

## Callers and boundaries

Identity will call key generation, signing and encryption while owning persistent
credentials, contacts, recovery and protected key-store installation. Cryptography
owns transient secrets and operations over exact bytes. It can validate Identity's
ID formula without calling a running Identity service.

Transport owns transcript construction, key labels, direction counters and replay
protection; a later composition adapter will implement its existing crypto port.
Content codecs own service-specific constructions, metadata and manifests. The
generic identity envelope does not replace Messaging's envelope or supply a chunk
ordering protocol. These boundaries prevent trust, persistence and transport state
from becoming hidden dependencies of primitive operations.

Local composition will provide secure entropy and authorized protected key access,
using established cryptographic libraries. This scaffold supports direct
in-process contracts only; it does not claim process-separated deployment.

## Public contracts

- [Primitives](primitives/mod.md): hashing, keys, signatures, agreement and derivation.
- [Credentials](credential.md): exact ID derivation and persistent-key binding.
- [API](api.md): capabilities, borrowed requests and owned results.
- [Resource limits](config.md): immutable per-operation byte budgets.
- [Errors](error.md): distinct secret-free failure categories.
- [Secrets](secrets.md): purpose-scoped ownership and cleanup.
- [Entropy](ports.md): fallible injectable platform randomness.

See [secret ownership](secrets.md), [platform randomness](random.md),
[hashing](primitives/hash.md) and [test support](test_support/README.md).
Cargo.lock pins deliberate established-library dependencies; no key store exists yet. Logging uses existing `tracing` configuration and records no input bytes.

## Build, exercise and test

From the repository root:

```sh
cargo build --locked
cargo test --locked modules::cryptography
cargo test --locked --doc
cargo doc --locked --no-deps
cargo run --locked -- --help
cargo run --locked -- help serve
```

There is no Cryptography CLI command yet. `cargo run --locked -- serve` still
exits with status 1 and opens no listener because real Identity/Cryptography
integration is absent. See [coverage instructions](../../../docs/coverage.md).

## Upstream baseline

Reviewed against [CR2SE revision 80f6795864ecf060e23482ee42909da4a649ecf0](https://github.com/beothorn/CR2SE/tree/80f6795864ecf060e23482ee42909da4a649ecf0).
Use its [specification](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/cryptography/README.md),
[input contract](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/cryptography/inputs/README.md),
[output contract](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/cryptography/outputs/README.md),
[pseudocode/vector](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/cryptography/pseudocode/README.md)
and [acceptance suite](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/cryptography/acceptanceTests/README.md).
Scaffold tests are boundary checks, not CR-01–CR-11 conformance evidence.
