# Services authority

Trusted composition owns Authority and issues opaque grants bound to one local
identity, action and positive byte budget. Actions are Register (registry changes),
Observe (inspection), Coordinate (accepted work), Invoke (provider calls), Request
(local requester input/spending) and Recover (durable recovery).

Checks reject foreign, dropped, revoked, wrong-identity or wrong-action grants.
Excess bytes fail with Limit. Revocation is permanent and never erases obligations.
Tokens are process-local and cannot be supplied in peer JSON. Grants confer no
Ledger, Identity or handler authority by themselves. Composition validates peer
provenance before passing authenticated call context.
