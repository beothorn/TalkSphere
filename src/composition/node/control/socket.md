# Control socket server

`start(layout, limits, signal, handler)` binds `control.sock` and serves requests.

Startup refuses a node root that is readable by others, refuses a socket another node still answers
on, and replaces a stale socket file. The socket gets mode 0600. Each connection runs on its own
worker thread, up to `control.connections`; extra connections receive one `busy` error and are closed.
Requests on a connection are served in order. While the node is stopping, new connections and new
requests receive `busy`. A client that disconnects mid-request does not affect others. Dropping the
server stops accepting, waits for workers and removes the socket file.
