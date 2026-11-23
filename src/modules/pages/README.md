# Static Pages

CR2SE [Static Pages](https://github.com/beothorn/CR2SE/blob/73edf2dc230bccb0f27f2ea6e06e938fff738862/modules/pages/README.md) (`cr2se.page` version 1, publisher-served) and the pages of the
[Hosted Publications](https://github.com/beothorn/CR2SE/blob/53f12451859e0ad207ade9b8a23be20ceb404495/modules/hosted-publications/README.md) proposal ([beothorn/CR2SE#14](https://github.com/beothorn/CR2SE/issues/14)).
Planned in [milestone 15](../../../plans/15_Static_pages/PLAN.md).

**Status:** in progress (plan steps 17-20: canonical paths and media types, the page manifest, hosted page building and assembly, and the publisher-served `cr2se.page` service with its terms, codec, local check, resource map, definition, provider adapter and client, tested through the real Services coordinator and Ledger; see [the service guide](service/mod.md)). The node binding, commands and exercise follow.

**Relationships.** Callers: node composition and the hosted publication service. Dependencies: hosted publications (the page snapshot is a publication kind), Services, Identity and Cryptography.
Canonical paths and media types are shared by both page flavors, which is why they live here and not in hosted publications.

**Build and test** from the repository root: `cargo build --locked`, `cargo test --locked --lib modules::pages`.
