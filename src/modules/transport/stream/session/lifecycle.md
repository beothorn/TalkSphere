# Session lifecycle operations

This file implements Session.end, cancel, sent, close and is_closed. Contracts
remain in the [session guide](../session.md): local and remote halves are
independent, normal completion requires remote END and fully written local END,
terminal outcomes occur once, cancellation stops production, and connection
closure fails unfinished work while preserving completed outcomes. Peer GOAWAY
reasons remain typed connection outcomes. Internal helpers are not caller APIs.

protocol_error validates the exact stream owner/connection and active state before
emitting existing stream ERROR with PROTOCOL_ERROR. Only the affected stream's
production is stopped; its unsent OPEN is retained to preserve numbering barriers.
The terminal event is local Failed(ProtocolViolation), and unrelated streams remain
usable. Closed/terminal/foreign handles or unavailable response capacity are typed
errors. No connection-wide control capability is exposed to application consumers.
