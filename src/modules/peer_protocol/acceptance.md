# Acceptance and durable reservation

Admission supplies separate Services quote and Coordinate contexts, an explicit
Current checker and the real local Coordinator. Peer input never contains these
grants. The coordinator's accepted operation remains independent of the stream.

Issuer.accept consumes an Issued quote only for its original owner, authenticated
association, finite expiry and exact accept quoteId plus SHA256 of original JSON.
Services rechecks current terms and consumes that quote generation once. Only then
does Coordinator.admit bind the original operation and obtain one finite hold.
Incoming retains the original association, provisional input, Selection and Price.
It is not complete input and grants no early execution. Rejected exchanges release
their original transient quote; durable admissions are outside transient cleanup.

Client.reserve requires its own opaque Consented token. It freezes the same exact
bytes and independently calculated price through Services acceptance, rechecks
current terms, admits/reserves the original key, supplies the complete locally
owned input and durably marks one outgoing dispatch. Only after all these succeed
does Outgoing expose accept JSON, original association, Invocation and input.
Repeated local negotiations cannot produce a second dispatch or additional hold.

Capacity observed at quote creation is no guarantee. Refusal at acceptance is
permitted; durable uncertainty remains attached to the original key for recovery.
No error or dropped token means rollback. Failed/lost sends after Outgoing exists
retain the hold and dispatch journal; do not automatically resend accepted work.
The provider still requires all body bytes, input.end and requester END before
supply/dispatch. Coordinator and Ledger own abort/settlement/recovery decisions.
