# Control client

`call(socket, request, timeout)` connects to the node's control socket, sends one request line and
returns the decoded reply: a result or an error code with its message. A missing or unreachable
socket, a node that closes without replying, and oversized or malformed replies (more than 4 MiB) are
reported as failures rather than panics.
