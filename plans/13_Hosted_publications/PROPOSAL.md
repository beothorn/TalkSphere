# Hosted Publications proposal record

Status: pushed to CR2SE and filed as [beothorn/CR2SE#14](https://github.com/beothorn/CR2SE/issues/14); upstream acceptance pending.

Base: [CR2SE 73edf2d](https://github.com/beothorn/CR2SE/commit/73edf2d). The proposal adds three commits (titles only; their
commits [0ddc95e](https://github.com/beothorn/CR2SE/commit/0ddc95e), [2b38974](https://github.com/beothorn/CR2SE/commit/2b38974), [53f1245](https://github.com/beothorn/CR2SE/commit/53f1245)):

1. Add independently generated Hosted Publications vectors.
2. Specify Hosted Publications: provider-hosted signed pages and feeds.
3. Link Hosted Publications from the CR2SE indexes and decisions.

New optional service `cr2se.hosted-publication` version 1 under `modules/hosted-publications/`. Operations: `store`, `list`,
`get`, `renew`, `remove` and the byte check. The signing bytes, page manifest bytes and restricted envelope are fixed by vectors
`V1` (public resource), `V2` (public page manifest) and `V3` (restricted feed post). Open questions for upstream review: whether
the envelope should hide the audience's encryption keys from hosts, whether a retraction tombstone is wanted, and whether a
cross-host latest-version hint is needed.

## Drafted upstream issue

Title: Proposal: Hosted Publications so phones can publish pages and feeds

Body: Static Pages and Public Feeds require the provider to be the publisher, and CR2SE has no relay, so a publisher that cannot
accept connections cannot publish. Proposal: a new optional service, `cr2se.hosted-publication` v1, where the publisher signs a
publication (page manifest, page resource or feed post, public or encrypted for chosen readers) and stores it under
Messaging-style paid placements at any providers; readers list, fetch and verify from any of them. Reuses Messaging placement,
pricing, renewal, release-credit removal and byte checks; page paths and media types from Static Pages; message bytes from Public
Feeds. Existing Pages and Feeds are unchanged. Includes inputs, outputs, pseudocode, diagrams, acceptance tests HP-01..HP-15 and
independently generated vectors. The change is available on request as commits on top of 73edf2d.

Filed on the owner's go-ahead after searching open and closed issues for an existing proposal (none).
