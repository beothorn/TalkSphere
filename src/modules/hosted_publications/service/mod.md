# Hosted publication Services bindings

The layer between Services and the hosted publication engine. Composition registers one `Client` (the requester contract and economics, used by both roles) and optionally one `ServiceProvider` with the Services registry after the participant has completed recovery (crash-recovery
resumption is added by a later plan step).

| File | Contract |
| --- | --- |
| [Provider](provider.md) | Stage, prepare, finalize and commit through Services |
| [Client](client.md) | Requester contract, exact pricing and check rules |

Startup order for a provider: recover the participant deferring to Services (`DeferToServices`), construct the `ServiceProvider`, register it, open the coordinator, resume its pending work, then publish the offering.
The offering is published only when the participant is ready and a handler is registered.
