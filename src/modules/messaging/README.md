# Messaging

Paid retention of signed messages for one recipient: CR2SE module [Messaging](https://github.com/beothorn/CR2SE/blob/73edf2dc230bccb0f27f2ea6e06e938fff738862/modules/messaging/README.md),
service `cr2se.messaging` version 1, pricing model `cr2se.messaging.v1`. Optional in CR2SE's minimum profile, required by TalkSphere
(see the [plan](../../../plans/11_Messaging_module/PLAN.md)).

**Status:** in progress (plan steps 1-29, 32 and 34 done). The provider engine is implemented and tested: validation, pricing,
the durable participant with recovery, store, discover, recover (with receipt and cached retry), status, renew, remove (with
once-only release credits), paid byte checks, per-identity limits, reclaim, and the recipient's authenticate/deduplicate/decrypt
step. Not yet implemented: the Services provider adapter and registration (step 30), the Peer Protocol requester (31), crash
recovery across Services and Ledger files (33), authenticated multi-process exercises (35) and the exercise guide (36). Nothing is
registered with Services or advertised on a Board until the provider is complete, so a node never offers an unfinished capability.
See [ACCEPTANCE.md](ACCEPTANCE.md) for the interim evidence per case and [REQUIREMENTS.md](REQUIREMENTS.md) for the reconciliation.

Build and test from the repository root with `cargo test --locked --lib modules::messaging` (and `modules::ledger` for the release
credit support).

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
