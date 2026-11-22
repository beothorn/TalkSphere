# Hosted pages and feeds specification plan

Status: planned for review; no step is implemented yet.

This is roadmap milestone 14 and planning folder 13. It is CR2SE specification
work, not TalkSphere code. The TalkSphere Static Pages and Public Feeds modules
(milestones 15 and 16) are planned after this specification is accepted.

## Problem

In CR2SE today the publisher's own identity serves its Static Pages and Public
Feeds, and a provider must refuse to serve feed messages signed by anyone else.
CR2SE has no relay. A phone behind NAT cannot accept a reader's connection and
is often offline, so profile pages and feeds between phones cannot work.

Direction agreed for review: a personal page or a feed post is treated like a
message. The publisher signs it and stores it at as many providers as it wants,
encrypted when it is not public, and readers fetch and verify it from any of
them. A page differs only in that its content is a simple static page. The
TalkSphere implementation should reuse the Messaging implementation.

## Steps

- [ ] **1.** Inventory what Messaging, Static Pages and Public Feeds specify about ownership, hosting, signing and encryption.
- [ ] **2.** Write the problem statement and requirements for publishers that cannot accept connections.
- [ ] **3.** Define a publisher-signed publication that any provider can host and any reader can verify.
- [ ] **4.** Define public and reader-restricted visibility with layered encryption.
- [ ] **5.** Define hosted placement, retention, renewal and removal by reusing Messaging placement rules.
- [ ] **6.** Define reader listing, retrieval and cursors for hosted publications.
- [ ] **7.** Define a hosted page as a publication carrying a static page snapshot.
- [ ] **8.** Define a hosted feed as an ordered series of publications.
- [ ] **9.** Define updates, replacement and the latest version across several providers.
- [ ] **10.** Define pricing, Board offerings and service definitions.
- [ ] **11.** Decide how the change is versioned against existing Pages and Feeds.
- [ ] **12.** Add pseudocode, input and output contracts and acceptance tests.
- [ ] **13.** Add independently generated fixed vectors for the new signing and encryption bytes.
- [ ] **14.** Update Redundancy, ModuleContracts, Glossary and PoC.
- [ ] **15.** Review the change for consistency, privacy and abuse.
- [ ] **16.** File the proposal upstream and record it locally.

## Baseline, scope and review

Planned on 2026-11-22 against
[CR2SE bc53df4](https://github.com/beothorn/CR2SE/tree/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16).

- [Public Feeds availability and ownership](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/modules/public-feeds/README.md#6-availability-and-feed-ownership).
- [Static Pages](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/modules/pages/README.md).
- [Messaging placements, signatures and encrypted payloads](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/modules/messaging/README.md).
- [Redundancy](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/Redundancy.md).

CR2SE is for every platform and language. All changes are language-independent:
exact byte layouts, pseudocode and acceptance tests, with no Rust types, crates
or tooling. Changes are made in a CR2SE checkout, committed there in focused
commits, and are not pushed without the user's request. Documents always link
CR2SE by GitHub URL, never by a local path.

The existing publisher-served Pages and Feeds remain valid for publishers that
can accept connections; step 11 decides whether hosting is a new mode, a new
service version or a new service.

## Details

### 1. Inventory what Messaging, Static Pages and Public Feeds specify about ownership, hosting, signing and encryption

Compare the three specifications: who serves content, what is signed, what is
encrypted, retention, pricing, checks and replay rules. List what Messaging
already provides that a hosted publication can reuse unchanged. Record the
inventory in the proposal draft.

### 2. Write the problem statement and requirements for publishers that cannot accept connections

State the phone case: no inbound connections, often offline, readers also
behind NAT. Require that readers can verify the publisher without trusting the
host, that hosting at several providers needs no coordination, and that hosts
learn no more than necessary.

### 3. Define a publisher-signed publication that any provider can host and any reader can verify

Define the publication metadata and signing bytes, following Messaging's
signature construction with a distinct domain string. A host validates the
signature before accepting it and serves the exact signed bytes.

### 4. Define public and reader-restricted visibility with layered encryption

Public publications are signed and unencrypted. Restricted ones encrypt the
content once with a content key and wrap that key for each authorized reader,
reusing Messaging's recipient encryption. Readers prove their identity to the
host to fetch the restricted ones. Define the exact layout and failure rules,
with decryption yielding nothing on failure.

### 5. Define hosted placement, retention, renewal and removal by reusing Messaging placement rules

Reuse Messaging placement IDs, acceptance and expiry, guarded renewal, removal
with release credits, byte checks and replay IDs. Only the publisher may renew
or remove. State each deviation from Messaging explicitly.

### 6. Define reader listing, retrieval and cursors for hosted publications

Define listing a publisher's hosted publications with an exclusive cursor, and
retrieving one, for public and restricted visibility, with exact bounds and
prices. Unauthorized readers learn nothing about restricted items.

### 7. Define a hosted page as a publication carrying a static page snapshot

Define the page snapshot: a canonical manifest of paths, media types and
content hashes, reusing Static Pages path and media-type rules, with resources
fetched by hash. Readers verify every resource against the signed manifest.

### 8. Define a hosted feed as an ordered series of publications

Define a feed post as a publication with Public Feeds message fields and
attachments, ordered by publisher sequence and time, with Feeds' selectors and
cursors adapted to hosting.

### 9. Define updates, replacement and the latest version across several providers

Define a publisher sequence number so readers pick the latest page version
across hosts, and how equivocation (two different items with one sequence) is
detected. Absence at one host proves nothing.

### 10. Define pricing, Board offerings and service definitions

Define exact rational pricing reusing Messaging's formulas, the Board offering,
required fields and the complete service definitions.

### 11. Decide how the change is versioned against existing Pages and Feeds

Decide between a hosted mode, a new service version or a new service identifier,
keeping existing publisher-served behavior valid. Record the decision and its
reasons in CR2SE's decision log.

### 12. Add pseudocode, input and output contracts and acceptance tests

Add module documents in CR2SE's existing layout: inputs, outputs, pseudocode,
diagrams and acceptance tests covering every rule above.

### 13. Add independently generated fixed vectors for the new signing and encryption bytes

Generate fixed vectors with an independent script, as done for Messaging,
covering signing bytes, manifests and layered encryption.

### 14. Update Redundancy, ModuleContracts, Glossary and PoC

Add hosted publications to the replica rules, module table, glossary and the
feature matrix.

### 15. Review the change for consistency, privacy and abuse

Check against existing Messaging, Pages and Feeds text for contradictions,
metadata leakage, unbounded listing and spam costs. Fix findings before filing.

### 16. File the proposal upstream and record it locally

Search open and closed CR2SE issues, file the proposal with links to the
changed documents, and record it under [CR2SE bugs](../../CR2SE%20bugs/README.md)
or a design note as appropriate. Report the URL; never claim filing without it.
