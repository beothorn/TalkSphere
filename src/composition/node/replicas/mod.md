# Replicas of hosted data

The node keeps the data it hosts as replicas at independent peers so that one unreliable peer (a phone) never decides
whether the data survives. Placement uses the [wanted offering](../wanted.md): clients earn the node's credits by
holding sealed copies. CR2SE Redundancy governs the accounting: a copy counts only after the provider returned the
complete successful Storage result and it validated, and confirmed copies are at distinct provider identities.

| Contract | Responsibility |
| --- | --- |
| [Sealer](sealer.md) | Authenticated encryption of an item with the node's data key |
| [Book](book.md) | Durable items, reservations and confirmed replicas |
| [Report](report.md) | CR2SE redundancy states from confirmed observations |
| [Maintenance](maintain.md) | Staggered checks, renewals and expiry of confirmed replicas |

Flow: `node-ctl replica-add --file F` seals the item and adds it to the book (the sealed bytes stay on the node so a repair
never depends on the replicas). Each client that offers to fulfil the wanted terms is given the item that most needs a
copy and that it does not already hold; the confirmed copy is recorded after the Storage result validated. Once an item
has the policy's `redundancy.target` copies (confirmed or in flight) it is no longer offered. `node-ctl replicas` lists
items and their confirmed replicas.
