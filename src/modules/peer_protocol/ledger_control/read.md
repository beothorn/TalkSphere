# Issuer read controls

respond takes receiving Invoke permission, real Ledger owner, separate Control
context, trusted authenticated Association, validated Message and finite Limits.
The local identity and original requester/connection/stream must match. No peer
input can select another local owner or administrative permission.

ledger.balance returns only credits this issuer recognizes for the authenticated
caller, ignoring optional owner hints. ledger.transfer.get decodes exact owner and
16-byte transfer ID and uses Ledger's original/new-owner access rule. Existing and
missing records are concealed as not_available for unrelated callers; invalid
local authority remains a typed error. A successful result preserves original
owner, new owner, transfer ID, gross, exact fee and checked positive net as canonical
uint64 strings. Complete control bounds are checked before returning a record.

No operation dispatches a service, grants/revokes credits, overwrites observed
owned balances or changes trust. Composition enforces one request/response,
control quotas and END in both directions through Exchange. Unknown Ledger state
or unavailable/invalid contexts cannot produce a successful observation.
