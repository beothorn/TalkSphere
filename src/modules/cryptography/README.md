# Cryptography

Status: step 1 defines contracts and validated local resource policy only.
There are no working cryptographic primitives, production entropy adapters or
envelope operations yet. The interfaces alone cannot authenticate networking.
See the [module plan](../../../plans/2_Cryptography_module/PLAN.md).

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

- [API](api.md): capabilities, borrowed requests and owned results.
- [Resource limits](config.md): immutable per-operation byte budgets.
- [Errors](error.md): distinct secret-free failure categories.
- [Secrets](secrets.md): purpose-scoped ownership and cleanup.
- [Entropy](ports.md): fallible injectable platform randomness.

Concrete secret types, deterministic test helpers and cryptographic algorithms
remain later steps. No algorithm or key-store dependencies were added for the
scaffold. Logging uses existing `tracing` configuration and records no input bytes.

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
