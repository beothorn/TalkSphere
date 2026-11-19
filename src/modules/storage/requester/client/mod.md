# Installed Storage requester

Client is a real Services Requester and paid-check Rules implementation for the
independently recognized Storage version-one definition. Construction takes the
complete Definition, a trusted standard Catalogue, an application-owned Leases
port, a requester Clock, and explicit Policy. It checks the standard and finite
value policy before retaining dependencies. It registers and publishes nothing.

Policy.values limits complete logical depth/nodes/bytes; zero usable budgets or
a depth outside 1..31 fail Invalid. Caller budgets intersect these limits and
cannot expand them. maximum_clock_difference_seconds is the explicit requester
receipt-time tolerance for store acceptance timestamps, in whole seconds.

Leases.lease receives the original operation key, actual provider, requester and
opaque lease ID. It returns an independently retained original-attempt lease
snapshot. The application must preserve that snapshot through later renew/remove
catalog changes and restart; the same ID at another provider is a distinct row.
The client checks the returned ownership/ID again. Missing or foreign resources
fail Denied; dependency failures remain typed. Credit issuer is never used as a
provider lookup key. These reads must be bounded, must not reenter Services or
wait for peer I/O, and must come from trusted local state rather than peer data.

Leases.range optionally returns exact private original range bytes for the same
attempt/provider/ID/offset/length. Absence is Ok(None), distinct from a failed
lookup. Full-object retrieval always checks SHA-256; a partial range without
original bytes verifies metadata/count only and establishes no range proof.
The client never writes the recovery catalog or exposes encryption keys through
these ports. Publish store/renew/remove catalog updates only after acknowledged
requester coordination, retaining original attempt snapshots for recovery.

The [Requester contract](contract.md) connects exact quotes and complete results
to existing Storage arithmetic/validators. [Input semantics](input.md) preserve
operation-specific field sets and body-free store metadata. [Check rules](checks.md)
use current positive paid terms and requester-only expected bytes/timing.
Context-free quote pricing/remote factor checks return Unsupported; authenticated
Services callbacks carry the original peer/key and selected offering. No missing
provider context is reconstructed from payload identities or issuer information.

This requester can be explicitly registered without a provider. Such registration
cannot make provided Storage selectable or publish a paid provider. Full step 28
still requires provider handlers and store creation after resolution of
[CR2SE #11](https://github.com/beothorn/CR2SE/issues/11). The normal executable
continues to expose its existing documented capabilities.

From the repository root:

```sh
cargo build --locked
cargo test --locked modules::storage::requester::client
cargo run --locked -- --help
```

Behavior follows pinned [Storage inputs](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/inputs/README.md)
and [outputs](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/outputs/README.md).

[Peer Protocol integration evidence](PEER.md) exercises these actual ports through
requester-only registration, original quote consent, complete result reception
and real local Services/Ledger decisions, with its adapter scope stated explicitly.
