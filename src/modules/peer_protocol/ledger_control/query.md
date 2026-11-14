# Authenticated issuer query observations

Request.balance and Request.transfer require local outgoing Request permission and
a trusted Association whose requester is the local caller and peer is the issuer.
They retain exact query bytes, original deadline and original owner/transfer tuple.
bytes borrows the bounded outgoing control.

finish requires separate incoming Invoke permission, the same authenticated peer,
connection and stream, the expected complete response and trusted normal completion.
Wrong attribution, explicit failure, missing/abnormal termination, changed tuple or
invalid gross/fee/net arithmetic cannot yield evidence. No binary body is allowed
by the outer exchange state machine.

Response.Balance is a uint64 observation only; it never overwrites the local book.
Response.Transfer owns an opaque Transfer proof. issuer reads the authenticated
responder, association reads the original query route, and result borrows the
validated original order/fee. The issuer is never taken from a forwarded JSON
field. The local caller must be original or new owner. Producing this proof alone
does not settle payment; later settlement must match the exact original invocation
and independently supplied Evidence/Coordinate grants.
