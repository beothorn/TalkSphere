# Messaging

Paid retention of signed messages for one recipient: CR2SE module [Messaging](https://github.com/beothorn/CR2SE/blob/73edf2dc230bccb0f27f2ea6e06e938fff738862/modules/messaging/README.md),
service `cr2se.messaging` version 1, pricing model `cr2se.messaging.v1`. Optional in CR2SE's minimum profile, required by TalkSphere
(see the [plan](../../../plans/11_Messaging_module/PLAN.md)).

**Status:** complete for the planned module scope (plan steps 1-38). Implemented and tested: validation, pricing, the durable
participant with recovery, every operation (store, discover, recover with receipt and cached retry, status, renew, remove with
once-only release credits, paid byte checks), per-identity limits, reclaim, the Services provider adapter and requester client with
Ledger release issuance ([service](service/mod.md)), crash recovery across independent Services, Ledger and Messaging files, the Peer
Protocol requester intents ([wire](wire.md)), the recipient's authenticate/deduplicate/decrypt step, Identity-backed payload
decryption ([identity](identity.md)) and an authenticated four-node exercise with replicas ([guide](../../../docs/messaging/EXERCISE.md)).
Nothing is advertised on a Board by default: a node installs the handler and publishes the offering only through its composition
(client journey, plan 14 step 2).
See [ACCEPTANCE.md](ACCEPTANCE.md) for the interim evidence per case and [REQUIREMENTS.md](REQUIREMENTS.md) for the reconciliation.

Build and test from the repository root with `cargo test --locked --lib modules::messaging` (and `modules::ledger` for the release
credit support); the real-endpoint tests are `cargo test --locked --lib endpoint::tests::messaging`. The authenticated exercise is
`cargo run --locked --example messaging -- run` (`cargo test --locked --example messaging`; see `cargo run --locked -- help messaging`).

A message is a signed association of metadata and one immutable payload. A sender stores it at a provider for one recipient; the
recipient discovers and recovers it with a signed request and gets a provider-signed receipt; the sender can check status, renew,
remove for early-release credits and challenge one payload byte. One placement is one copy at one provider: redundancy is the
sender's job (see [CR2SE Redundancy](https://github.com/beothorn/CR2SE/blob/73edf2dc230bccb0f27f2ea6e06e938fff738862/Redundancy.md)).

**Relationships.** Callers: node composition (registration, settlement ports) and the Services coordinator. Dependencies: Services
(contracts, replay IDs), Identity and Cryptography (key derivation, signatures, encryption), Ledger (balances and release credits),
Board (offering and the `identityEncryption` credential) and Peer Protocol (wire binding). It owns placements, receipts and replay results;
its persistence is local and independent of the paid Storage service and of Discovery. Boundaries are small ports so peer policy
decisions (release amounts, per-sender and per-recipient limits) are filled by composition, never hard-coded here.

**Build and test** from the repository root: `cargo build --locked`, `cargo test --locked messaging`. Further exercise commands are added with
the later steps.
