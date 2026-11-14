# Independent Ledger controls

This Peer Protocol subdomain maps authenticated bounded controls to Ledger public
ports. It owns wire parsing, original stream correlation and normal-completion
provenance. Ledger owns signatures, balances, durable transfers, claims and payment
replay guards; Services dispatch and administrator grants are not involved.

read serves caller-scoped balance/protected transfer queries. query creates
original outgoing requests and returns observations only after matching responses
and normal completion. Later transfer, claim and settlement bindings share those
validated canonical controls without exposing Ledger private tables.
