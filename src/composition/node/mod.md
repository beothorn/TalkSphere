# Minimum node composition

This module composes the eight required CR2SE modules into one node, below `src/composition/node/`.
It is Linux-only and does not change the ordinary `serve` composition.

| Contract | Responsibility |
| --- | --- |
| [Layout](layout.md) | Fixed paths inside one node root |
| [Configuration](config.md) | Validated operator policy in `node.json` |
| [Adapters](adapters.md) | Native clock, entropy and hashing for Storage |
| [Catalog](catalog.md) | Durable requester lease catalog |
| [Offering](offering.md) | Validated Storage offering and definition |
| [Owners](owners.md) | Open, create and recover every durable owner |
| [Initialization](init.md) | Explicit creation and read-only inspection of a node root |
| [Observation store](observation_store.md) | Durable bounded per-peer observations for peer policy |
| [Link](link.md) | Command queue between control workers and the node loop |
| [Runtime](runtime.md) | Startup order, event loop and shutdown drain |
| [Control](control/mod.md) | Owner-only local control socket and commands |
