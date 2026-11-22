# Default policy

`Defaults` implements every contract as the minimum node behaves: admit every authenticated peer,
allow 8 pending operations per identity with no rate limits, leave trust unchanged, issue no
bootstrap credits, publish no wanted offering, offer exactly the requested minimum as release
credits, target 2 replicas, check each replica hourly staggered by a tenth of an hour per replica
index, ping every 30 seconds with a 10 second timeout, and reconnect after 1, 2, 4, ... seconds
capped at 60. `Defaults::policy()` returns the bundle.
