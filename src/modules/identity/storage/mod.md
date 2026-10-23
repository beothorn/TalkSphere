# Identity platform storage

The module exports FileStore for protected Linux filesystem access. Other targets
export an explicit unsupported adapter whose constructor fails without I/O.
The adapter owns file descriptors, locks and permission checks. Identity owns
credential validation and lifecycle; Cryptography owns key operations. Neither
paid Storage nor Discovery is a dependency.
