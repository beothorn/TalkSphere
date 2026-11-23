# Hosted Publications

Provider-hosted, publisher-signed pages and feed posts: the CR2SE proposal
[Hosted Publications](https://github.com/beothorn/CR2SE/blob/53f12451859e0ad207ade9b8a23be20ceb404495/modules/hosted-publications/README.md)
([beothorn/CR2SE#14](https://github.com/beothorn/CR2SE/issues/14), upstream acceptance pending), service `cr2se.hosted-publication`
version 1, pricing model `cr2se.hosted-publication.v1`. Optional in CR2SE's minimum profile; planned in
[milestone 15](../../../plans/15_Static_pages/PLAN.md).

**Status:** in progress (plan steps 1-16 of [milestone 15](../../../plans/15_Static_pages/PLAN.md)). Implemented and tested: signing with the V1-V3 vectors, verification, the restricted envelope, terms and exact prices, the durable store and lifecycle, store, list, get, renew, remove with once-only release credits and the byte check, the Services codec, the pinned definition, the participant with recovery, the Services provider adapter and requester client, and crash recovery across Services, Ledger and hosted files. Pages, the node binding, commands and the exercise follow. Nothing is advertised on a Board by default.

A publisher that cannot accept connections (a phone) signs a publication and stores it, for a paid retention period, at any number of
hosts. Readers list and fetch it from any host and verify the publisher's signature themselves, so a host can withhold or serve an
older version but cannot forge content. Restricted content is encrypted for a chosen audience.

**Relationships.** Callers: node composition, the Services coordinator and the pages and feeds modules. Dependencies: Services,
Identity and Cryptography (identity IDs, signatures, X25519 and AEAD), Ledger (balances and release credits), Board (the
`identityEncryption` credential and the offering) and Messaging only through its public pricing helpers, ports and identifier types,
because placement, retention, renewal and removal rules are shared by reference. The module owns its placements and replay results;
its persistence is local and independent of every other paid service.

**Build and test** from the repository root: `cargo build --locked`, `cargo test --locked --lib modules::hosted_publications`.
