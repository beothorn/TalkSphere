# Transport control driver

`Driver.new(config, dependencies)` owns validated policy, socket provider, clock,
Identity and Cryptography capabilities. It creates shared admission/byte budgets
and a local protocol registry; no socket is opened at construction. `resources`
exposes current occupancy without authority or payloads.

The driver implements TransportControl. `submit` validates capabilities and
limits, returns immediate admission or typed rejection and wakes a pending poll.
Connect attempts reserve capacity before socket initiation and have deadlines;
Accept transfers a socket into bounded handshake composition. Listen reports
its actual bound address; StopListening closes it and queues an outcome.
Registration validates exact protocols and issues owner authority; at most 256
local registrations are retained. Stream, Ping, Drain and Close route only to
the correct instance and owner. No peer can invoke local registration.

`poll_event` advances nonblocking attempts, incoming admission and round-robin
connection work, then transfers one observation. It registers the earliest
connection/probe deadline with the injected clock. Pending means the driver is
idle with wakeups registered; it remains available for future submissions.
Connection capacity also bounds closed instances retaining undelivered outcomes.
Listener counts and driver event queues are finite. Consumers must continue
polling to release observations and recover admission capacity.

TCP completion does not authenticate identity. Real module adapters must be
supplied before running an authenticated deployment. Test substitutes are
compiled only for tests, with no CLI switch to enable them.
