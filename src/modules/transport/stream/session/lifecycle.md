# Session lifecycle operations

This file implements Session.end, cancel, sent, close and is_closed. Contracts
remain in the [session guide](../session.md): local and remote halves are
independent, normal completion requires remote END and fully written local END,
terminal outcomes occur once, cancellation stops production, and connection
closure fails unfinished work while preserving completed outcomes. Peer GOAWAY
reasons remain typed connection outcomes. Internal helpers are not caller APIs.
