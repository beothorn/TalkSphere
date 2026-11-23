# Hosted publication Services bindings

The layer between Services and the hosted publication engine. Composition registers one `ServiceProvider` with the Services registry after the participant has completed recovery (the requester `Client` and
crash-recovery resumption are added by later plan steps).

| File | Contract |
| --- | --- |
| [Provider](provider.md) | Stage, prepare, finalize and commit through Services |

Startup order for a provider: recover the participant deferring to Services (`DeferToServices`), construct the `ServiceProvider`, register it, open the coordinator, resume its pending work, then publish the offering.
The offering is published only when the participant is ready and a handler is registered.
