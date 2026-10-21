# Cryptography

Status: all 22 planned steps are complete; verification evidence is recorded in
the [acceptance matrix](ACCEPTANCE.md) and [module plan](../../../plans/2_Cryptography_module/PLAN.md).
Primitives are exercised through local library calls; they do not start a server.
Identity persistence and authenticated server composition remain later work.

## Callers and boundaries

Identity will call key generation, signing and encryption while owning persistent
credentials, contacts, recovery and protected key-store installation. Cryptography
owns transient secrets and operations over exact bytes. It can validate Identity's
ID formula without calling a running Identity service.

Transport owns transcript construction, key labels, direction counters and replay
protection; the real composition adapter implements its existing crypto port.
Content codecs own service-specific constructions, metadata and manifests. The
generic identity envelope does not replace Messaging's envelope or supply a chunk
ordering protocol. These boundaries prevent trust, persistence and transport state
from becoming hidden dependencies of primitive operations.

Local composition supplies secure platform entropy and authorized protected key access,
using established cryptographic libraries. The implemented provider supports direct
in-process contracts only; it does not claim process-separated deployment.

## Public contracts

- [Primitives](primitives/mod.md): hashing, keys, signatures, agreement and derivation.
- [Credentials](credential.md): exact ID derivation and persistent-key binding.
- [Provider](provider.md): real in-process composition of the public contracts.
- [Transport adapter](../../composition/cryptography.md): real primitive port mapping.
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
exits with status 1 and opens no listener because Identity and normal server
composition are absent. See [coverage instructions](../../../docs/coverage.md).

## Upstream baseline

Reviewed against [CR2SE revision 80f6795864ecf060e23482ee42909da4a649ecf0](https://github.com/beothorn/CR2SE/tree/80f6795864ecf060e23482ee42909da4a649ecf0).
Use its [specification](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/cryptography/README.md),
[input contract](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/cryptography/inputs/README.md),
[output contract](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/cryptography/outputs/README.md),
[pseudocode/vector](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/cryptography/pseudocode/README.md)
and [acceptance suite](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/cryptography/acceptanceTests/README.md).
The acceptance matrix links fixed-vector, failure, lifecycle and cross-module tests.
Protocol findings 0003/0004 are fixed upstream; see the [bug index](../../../CR2SE%20bugs/README.md).
Identity steps 3–4 adopt and test the updated signature profile. Broader cleanup
and Identity storage acceptance additions remain tracked by the Identity plan;
these results do not establish unconditional interoperability or node conformance.

## Limits, dependencies and Identity handoff

Callers choose immutable plaintext/object budgets; the Provider ceiling must be
at least 148 bytes and intersects every envelope request budget. An object adds
132 header bytes and a 16-byte tag to its plaintext. Length checks precede
allocation and expensive work. Platform entropy requests are bounded at 4096
bytes; HKDF expansion is bounded at 255 SHA-256 blocks. No implicit network retry
or plaintext streaming is provided.

The implementation uses sha2, ed25519-dalek, x25519-dalek, hkdf/hmac and
chacha20poly1305 with getrandom for entropy and zeroize for owned secret cleanup.
Cargo.lock records exact versions. Purpose-specific capabilities prevent accidental
key reuse across operations; ephemeral authority is consumed and transient keys
are erased on exit. This does not guarantee erasure of every compiler/backend
copy. Generic envelopes provide neither sender authentication nor forward secrecy;
Transport integrity does not encrypt its payloads.

Identity must next create and protect persistent credentials, authorize private
operations, handle installation/conflicts and supply the Transport Identity port.
Server composition then wires those credentials, the real Cryptography adapter,
TCP and lifecycle management into `serve`. The module's tested in-process provider
can already be used by library callers; its generated API documentation includes
an executable recipient-encryption example. Run
`cargo test --locked composition::cryptography` for real handshake/vector checks
and `cargo test --locked --test cryptography_privacy` for trace privacy.

Identity dependency work adds [canonical verification-key validation](primitives/verification_key.md)
for the updated d40b6278 profile. It uses curve25519-dalek directly (already used
by the signing backend) for decode/re-encode and small-order checks. Signature
alignment now follows the same profile; see the acceptance supplement.
