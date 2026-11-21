# Control protocol

One request is one JSON object on one line: `{"id": <unsigned integer>, "op": "<name>", ...parameters}`.
The reply is one line echoing the id: `{"id":N,"ok":true,"result":{...}}` or
`{"id":N,"ok":false,"error":{"code":"...","message":"..."}}`. Numbers are exact unsigned JSON
integers (negative, fractional or text numbers are refused); byte values are lowercase hex. Request
lines are bounded by `control.lineBytes`; an oversized line gets one error and the connection is closed.
The id doubles as the idempotency key for `grant`.

Error codes: `invalid`, `unsupported`, `denied`, `refused` (peer refused before any commitment),
`insufficient`, `unavailable`, `limit`, `unknown_outcome` (the remote effect may have committed and
the work is never retried) and `busy` (stopping or too many connections). Messages never contain
secrets, object content or peer-supplied text.

`parse` turns a line into a request; the `Request` accessors read required or optional integers,
text and bounded hex parameters and report `invalid` with the parameter name. `read_line` reads one
bounded line; `ok` and `error` encode replies; `hex`/`unhex` convert bytes.
