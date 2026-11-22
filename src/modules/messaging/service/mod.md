# Messaging Services bindings

The layer between Services and the Messaging engine. Composition registers one `Client` (the requester contract and economics, used by both roles) and optionally one `ServiceProvider` with the Services registry, and installs a `LedgerIssuer` on the participant.

| File | Contract |
| --- | --- |
| [Provider](provider.md) | Stage, prepare, finalize and commit through Services |
| [Client](client.md) | Requester contract, exact pricing and check rules |
| [Release](release.md) | Release credit issuance through the Ledger |

Startup order for a provider: recover the participant deferring to Services (`DeferToServices`), have the coordinator resume its pending Messaging work (it re-delivers recorded decisions with their retained decision times), then register the handler and publish the offering. The module is published only when the participant is ready and a handler is registered; otherwise the Board refuses the offering.
