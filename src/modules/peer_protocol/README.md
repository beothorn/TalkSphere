# Peer Protocol

Peer Protocol binds `cr2se.peer` version1 to authenticated Transport streams,
Board metadata, Services coordination and Ledger controls. Its38-step plan is
implemented; the [acceptance matrix](ACCEPTANCE.md) records executable evidence,
independent vectors, family substitution scopes and the unresolved upstream
paid-check timeout question. Paid Storage is the [next module](HANDOFF.md).

The module owns bounded record/control/value codecs, exact source quote hashes,
canonical numbers/bytes and ordered detached bodies. It composes provided/wanted
work, independent quote validation and explicit spending consent, original-key
family-authorized cached retries, challenge-only paid/included evidence and
Ledger balance/transfer/claim controls. Received results stay provisional until
complete bodies/markers, normal Transport completion and local commitment.

A [bounded worker pool](engine/worker.md) runs synchronous public dependency calls
outside the Transport reactor. The opt-in [stream bridge](engine/bridge/mod.md)
retains original authenticated scopes, deadlines/timing, bounded framing and late
cleanup. [Endpoint factories](engine/endpoint/README.md) join actual Board,
Registry, Services and Ledger owners under separate local authority. Shared
[admission budgets](engine/endpoint/resources.md) bound transient logical values,
body references and each authenticated peer across connections and clones.

Transport owns authentication, sockets and framed stream outcomes; Board owns
publication/provenance; Services owns accepted work and participant coordination;
Ledger owns balances, holds and permanent replay. Families own operation-specific
schemas, formulas, authorization, durable resources and evidence. Peer Protocol
never reads another owner's private maps, installs code from remote metadata or
infers rollback from lost delivery. Work and third-party payment remain separate.
Trusted composition supplies protected key ports and application consent; absent
capabilities refuse explicitly. Default `serve` retains its Transport-only profile.

From the repository root:

```sh
cargo build --locked
cargo run --locked -- --help
cargo run --locked -- help peer-protocol
cargo run --locked --example peer -- --help
cargo run --locked --example peer -- help run
cargo run --locked --example peer -- run
cargo test --locked modules::peer_protocol
cargo test --locked --example peer
cargo test --locked --test peer_protocol_streams
```

The example runs two actual authenticated nodes with an explicitly installed
volatile custom text service, spending seven of100 credits on each independent
book. It is not a standard family or a persistent minimum useful node. Its help
and the [runtime guide](../../../docs/peer-protocol/RUNTIME.md) explain exact
configuration, local authority, protected initialization and recovery boundaries.

[Real-node exercises](../../../docs/peer-protocol/NODES.md) additionally cover
detached multibyte values, wanted roles, paid/included evidence, full independent
third-party settlement, three actual processes and protected issuer-record restart:

```sh
cargo test --locked --lib modules::peer_protocol::engine::endpoint::tests::network -- --nocapture
```

The separate TCP integration test checks metadata and malformed-sibling isolation.
These are same-implementation instances, not independent interoperability evidence.
Custom network-test service resources/journals are explicitly volatile; the issuer
restart test uses its actual protected durable Ledger. Services and Ledger retain
their separate complete durable participant recovery suites.

See the [plan](../../../plans/8_Peer_Protocol_module/PLAN.md),
[requirements](REQUIREMENTS.md), [contracts](mod.md), [Storage handoff](HANDOFF.md)
and [pinned upstream specification](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/peer-protocol/README.md).
The open [paid-check timeout issue10](https://github.com/beothorn/CR2SE/issues/10)
limits global no-charge claims after provider commitment/response loss; it does
not authorize refunds, automatic retries or an invented wire acknowledgement.
