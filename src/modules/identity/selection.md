# Identity selection and recipient conflicts

select_configured takes a list of complete public credentials and returns its sole
validated member. Zero or multiple entries are invalid, including repeated copies
of the same identity. Node composition must load the corresponding private handle;
a public selection alone cannot start an authenticated node.

classify validates both credentials and returns Unchanged, NewIdentity (different
signing ID), or InconsistentEncryptionKey (same ID, different persistent key).
require_unchanged accepts only Unchanged and returns Conflict for either change.
It does not permit transparent replacement or invent a version-one rotation protocol.

RecipientPins.new requires a positive entry budget. retain validates the binding
and ID before inserting; a previously retained ID requires unchanged encryption
authority, even when a competing binding has a valid signature. At capacity, a new
ID returns ResourceLimit. get returns a copied optional public credential. Failure
does not replace an old entry. The cache is local and in-memory, not Discovery,
trust, an authentication proof or a persistent rotation/selection registry.

Identical pairs copied to independent nodes produce identical authority. The PoC
allows one writer per protected local directory; it cannot coordinate unknown
machines or hard-drive copies. Copying private keys does not replicate Ledger,
Board, credits, relationships, catalogs or service commitments. Starting with a
new identity is an explicit new configuration, never inherited service state.
