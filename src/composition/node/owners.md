# Node owners

`open(layout, config, local, create)` opens every durable owner of one node, or creates
them fresh when `create` is set, and returns the recovered set. Order follows the Storage
handoff: Storage bytes and journal; the Ledger (initialized with the configured issuance
and internal budgets) and Services coordinator under the same identity; Storage participant
recovery against Services decisions; then the Board. Prepared effects resolve from durable
Services decisions before anything is reachable from the network. A failing owner stops
startup with a message naming the stage and no secret or payload; a second opener cannot
share the exclusive stores.

A provider handler is installed only when an offering is configured. A requester-only
node still installs the Storage requester client so it can buy from others. The Board
holds only derived publication state, so a withdrawn offering discards the stale snapshot
on reopen; the `storage/` directory is never touched, and its leases are not served until an
offering is configured again.

`Owners::publish` publishes the configured Board and definition (or an empty Board) after
`open` succeeded; it may be repeated and replaces the previous publication.

The result exposes the local identity, validated policy, Peer Protocol endpoint, Services
registry/runtime/authority, Ledger and its authority, and the requester lease catalog. The
Storage value budget is fixed (depth 12, 4096 nodes, 65536 bytes), which bounds objects to
16384 bytes in this profile.
