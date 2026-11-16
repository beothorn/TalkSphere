# Lease authorization

Authorize receives a candidate lease or absence, trusted local provider identity,
authenticated requester identity and the requested 32-byte lease ID. It returns
a borrowed immutable lease only when all three identifiers match.

Every inaccessible case returns Denied: absent resource, mismatched lookup,
foreign requester or different provider. State and expiration checks happen only
after authorization. No protected size, hash, times or state is returned on denial.
The operation creates no resources and changes no balances. Authentication is a
composition responsibility; payload identities cannot replace trusted context.
Connection IDs are deliberately irrelevant, allowing the same coordinated owner
to use a new authenticated connection. Knowing an ID or hash is insufficient.
