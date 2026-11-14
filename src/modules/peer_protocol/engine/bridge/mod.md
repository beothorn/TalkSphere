# Authenticated stream adapter

`Bridge` connects an explicitly installed trusted handler factory to Transport's
public control interface. It registers `cr2se.peer` version1, routes only its own
opaque stream capabilities, and runs dependency callbacks on fixed workers.
Registration requires valid finite configuration, system entropy and successful
Transport registration. Failure does not install a fallback implementation.

This is an opt-in library adapter. The default `serve` command remains Transport
only; a complete production factory wiring the existing Peer Protocol handlers
is still required. The unit-test echo factory is a framing probe, not a service
implementation and not authenticated network evidence.

`open` requires the original connection handle and an earlier verified
Authenticated event. `event` consumes owned observations and returns unrelated
protocol or node lifecycle events to composition. `poll` progresses asynchronous
work and output without invoking dependency code on the reactor. `stop` refuses
new streams while retaining existing work. `active` includes streams waiting for
late worker completion or closure cleanup. Stopping does not close the underlying
Transport connections; node composition owns their shutdown capabilities.

Every connection receives a fresh instance identifier consisting of an OS-random
192-bit process domain and a nonwrapping64-bit counter. Reconnecting to the same
identity with the same wire stream number cannot reuse the prior operation scope.
Requester roles are bound only after handler policy validates selected terms;
stream opener and requester are distinct for wanted work.

See [handler and resource contracts](contract.md), [event routing](events.md),
[scheduling](scheduling.md) and the [worker ownership contract](../worker.md).
From the repository root, build with `cargo build --locked` and exercise these
contracts with `cargo test --locked modules::peer_protocol::engine`. On Linux,
`cargo test --locked --test peer_protocol_streams` adds real authenticated TCP and
Board metadata evidence using an explicitly installed metadata-only test factory;
it does not demonstrate complete service-node composition or interoperability.
