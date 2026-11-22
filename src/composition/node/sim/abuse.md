# Abuse experiment

`run(&Abuse)` counts request attempts with the real rolling-minute limiter ([Rates](../../policy/rates.md)) at the limit the
real quota policy returns, one simulated second at a time. Abusive peers attempt `abuserRequestsPerSecond` requests every second;
honest peers make bursts (one fetch opens several streams) at `honestRequestsPerMinute` on average. The outcome gives the requests
per minute an abuser gets through, the share of abuser requests refused and the share of honest requests refused. With no limit
the abusers are unconstrained; with a limit they get about the limit per minute; a limit below honest demand refuses honest peers
too, which is the trade-off `quota.requestsPerMinute` tunes. Runs are reproducible from the seed.
