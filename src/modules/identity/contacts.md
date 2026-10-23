# Configured contact ownership

ContactBook.new(limits) returns an empty in-memory book and a book-specific opaque
ContactAccess capability for trusted local composition. Access may be cloned for
authorized local adapters; there is no peer representation. Another book's access
is rejected before lookup or capacity checks.

The book implements ContactStore: get returns a copied optional Contact by binary
ID; put validates endpoint shape and the canonical text budget before adding or
replacing that ID's hint. Adding beyond the contact-count budget fails; updating
an existing entry does not consume another slot. No other ID changes.

snapshot returns copied contacts in binary-ID order. replace validates a complete
bounded list, rejects duplicate IDs and publishes it only after all checks pass.
Denied, malformed, duplicate and oversized operations leave previous state intact.
Earlier snapshots never change. The implementation is explicitly in-memory;
durability is a later adapter and no Discovery, trust or network call occurs.
