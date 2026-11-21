# Minimum useful node handoff

The minimum useful node is complete for a finite Linux profile: two separately operated
`serve --node-root` processes buy and sell paid Storage over authenticated loopback TCP, driven locally
through an owner-only control socket. See the [acceptance evidence](../../../docs/minimum-node/ACCEPTANCE.md),
the [operator guide](../../../docs/minimum-node/RUNBOOK.md) and the
[plan](../../../plans/10_Minimum_node/PLAN.md). This is the minimum profile only: it is not the full-feature
PoC, not a CR2SE Node API and not interoperability with another implementation.

## Supported profile and limits

- Linux with comparable same-boot uptime; protected native Identity, Board, Services, Ledger and Storage
  owners; one active writer per node root (the identity lock).
- Objects of at most 16384 bytes (the logical value budget); one lease is one copy at one provider;
  encryption, replication and recovery catalogs are the operator's.
- Finite configured capacity, lease slots, pending operations and per-requester operations; a full
  provider refuses before work. Control: owner-only root and socket, bounded lines and connections.
- Credits are provider-issued: the operator records issued and owned credits and trust explicitly.
  Third-party settlement is a configuration flag the library supports but `node-ctl` cannot use.
- Only provider-issued payment, no wanted Storage, no Node API, Discovery or optional family.
- `grant` idempotency is remembered while the node runs, not across restarts; operations that end
  `unknown_outcome` are journaled but there is no remote status query to resolve them.

## Startup, recovery and shutdown

Start order is fixed: configuration, identity lock, owners (Storage, Ledger, Services, then Storage
participant recovery from Services decisions, then Board), publish, Transport, Peer Protocol, listener,
control. A failure at any stage stops startup naming the stage. SIGINT/SIGTERM refuse new work, let
accepted exchanges drain up to `--shutdown-ms`, and release the lock; a second signal forces closure.

## Contracts for the full client journey

- **Control channel**: line-delimited JSON over `control.sock` (`control/protocol.md`). New operations
  (wanted Storage, file or message flows) add operations there; the Node API would be a separate adapter.
- **Link**: workers never touch Transport; they use `Link` commands with deadlines.
- **Catalog**: `FileCatalog` implements the Storage requester lease ports; extend it, do not bypass it.
- **Ledger policy**: set in `node.json`; peers never select it. The full journey needs credit earning
  (wanted Storage) and trust grants, which need requester-owned consent that control does not yet carry.
- **Owners**: `owners::open` is the single place that opens and recovers the durable owners.

Production line coverage measured 94.12% with the [coverage procedure](../../../docs/coverage.md).

Milestone 12 (the full client journey) and the optional modules remain open.
