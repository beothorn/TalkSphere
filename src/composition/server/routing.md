# Local consumer routing

Router starts empty. register takes trusted Transport control, an exact protocol
selection and an owned Consumer, returning that registration's StreamOwner. Invalid,
duplicate or excessive registrations fail; at most 256 consumers are retained.
No peer can register code or obtain local authority. Production installs no fake
cr2se.peer, echo or debug protocol; unknown OPEN remains Transport's refusal.

route sends stream events only to the consumer whose owner capability authorizes
the event's stream. Unknown ownership returns Runtime before delivery. Non-stream
lifecycle events are returned to application composition. Transport must produce
these events only after authentication; Router is a local trust boundary, not an
independent wire parser. Consumer.event owns one observation and must apply bounded
retention. Failure requests cancellation of that stream and produces a fixed warning;
it does not retry application work or change Ledger state. Terminal cancellation
may already be unnecessary; a refused cancellation does not fabricate success.
