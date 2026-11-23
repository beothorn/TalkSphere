# Publisher-served Page service

`cr2se.page` version 1: the publisher's own identity serves one resource per paid `get`, at a fixed price, and the check is local and included.

| Contract | Responsibility |
| --- | --- |
| [Terms](terms.md) | Fixed price and path and resource limits of the offering |
| [Codec](codec.md) | Typed `get` request and output |
| [Check](check.md) | The visitor's local verification |
| [Store](store.md) | The publisher's resource map and snapshots |
| [Standard](standard.md) | Pinned definition and Services recognition |
| [Provider](provider.md) | Stage, prepare and commit through Services |
| [Client](client.md) | Requester contract, result validation and local check |
| [Wire](wire.md) | Peer Protocol requester intent with the exact fixed ceiling |

Startup for a publisher: build the resource source, check that the root is usable for the offering (`check_root`), register the `Client` and the `PageProvider`, then publish the offering.
