# Ledger authority

Trusted application composition owns an Authority and issues opaque Grant values
for one local identity, one action and a positive byte budget. Actions distinguish
administration, reads, coordination, authenticated controls and outgoing spending.
A receiver checks its own authority domain, exact identity, action and byte budget
before effects or protected-state disclosure. Foreign, dropped or revoked issuers
are denied; excess bytes return Limit. Revocation is permanent and does not erase
durable obligations. Recovery must receive separately authorized coordinator grants.

These tokens are process-local, never peer JSON or proof of peer authentication.
Possession of the authority issuer is privileged. Control adapters must validate
Transport authentication before requesting grants or constructing peer context.

Evidence is a separate privileged action for adapters asserting original-issuer
authentication; ordinary peer Control handlers cannot manufacture payment evidence.
