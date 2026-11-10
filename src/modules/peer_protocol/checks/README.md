# Remote checks

This Peer Protocol subdomain binds challenge-only paid/included checks to real
Services coordination and Ledger reservations. Callers provide authenticated
stream events, separate grants, clocks and explicitly installed family rules.
Families own eligibility and evidence; Services owns durable decisions; the
requester alone owns expectations and calculates pass/fail/inconclusive.

From the repository root:

```sh
cargo build --locked
cargo test --locked modules::peer_protocol::check
cargo run --locked -- --help
```

See [contracts](mod.md), [the module plan](../../../../plans/8_Peer_Protocol_module/PLAN.md)
and [the pinned wire specification](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/peer-protocol/README.md#check-binding-and-retries).
The test resource is an explicit custom fixture, not a Storage/Discovery/Messaging
provider. Real Transport composition remains a later step. Included allowance
consumption is reconciled separately in step27; paid response-loss ambiguity
remains recorded in [issue10](https://github.com/beothorn/CR2SE/issues/10).
