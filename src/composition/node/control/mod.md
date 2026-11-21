# Local node control

Authorized local control of a running node over an owner-only Unix socket `control.sock` inside
the node root. Authority is filesystem permission: the root must be owner-only, the socket is
created with mode 0600, and the channel is never reachable from the network. It is a TalkSphere
operator interface, not the CR2SE Node API.

| Contract | Responsibility |
| --- | --- |
| [Protocol](protocol.md) | Bounded line-delimited JSON requests and replies |
| [Server](socket.md) | Socket server, connection limit and shutdown behavior |
| [Client](client.md) | One request, one reply, used by `node-ctl` |
| [Commands](commands.md) | Accounting, inspection and paid Storage commands |
| [Journal](journal.md) | Recent operation outcomes, including unknown ones |

`start(layout, started)` builds the command handler for a ready node and starts the server.
