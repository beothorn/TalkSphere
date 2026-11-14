# Verified event routing

`Bridge.event` accepts trusted Transport events and returns events belonging to
other protocols or node lifecycle responsibilities. Only an Authenticated event
creates a connection observation. It must be a distinct instance and fit the
connection budget; on refusal, composition must handle the connection using its
own control capability. No application callback runs during routing.

A stream opens only when its opaque owner, connection instance, exact local/peer
context and selected protocol/version match. Bounds are checked before allocating
a decoder or output queue. The StreamOpened acknowledgement for a locally opened
stream confirms its existing entry without changing opener roles or invoking the
factory again. Refusal affects only the stream. Foreign protocol
observations return to composition instead of reaching a peer handler.

DATA is retained under per-stream aggregate pending/in-flight byte and observation
limits. Exceeding either requests resource-limit cancellation without retrying
application work. Remote END remains ordered after earlier DATA; its callback is
performed by a worker after record-boundary validation.

Normal StreamClosed observations retain earlier queued DATA and remote END until
they are processed. Only then is normal closure delivered to the handler. This
prevents a fast Transport event queue from discarding an unprocessed result.
Abnormal closure discards input that can no longer authorize fresh effects. An
already running callback is allowed to finish; newly produced output is discarded
and the actual closure is delivered afterward. The first closure remains final.

Connection closure removes its authenticated observation and closes affected
streams. A missing explicit failure on a still-active stream is treated as lost
connection, never as an application-success proof. Handler state is retained until
late work and closure cleanup finish, unless a worker panic has already lost that
transient state; original durable owners still retain their obligations.
