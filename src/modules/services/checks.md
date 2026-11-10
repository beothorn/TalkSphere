# Remote evidence checks

Rules is installed by the exact registered Requester; absence means local-only
checking. mode returns Local, Paid or Included(scope, maximum). Paid uses the
current offering's positive exact checkPrice and issuer. Included refers to an
existing paid entitlement with a stable owner-scoped identity and finite accepted
attempt count. That identity must survive offering, issuer and version updates;
changed limits conflict rather than silently granting more checks.

Rules validates immutable challenge shape, current authorization/resource
eligibility, response milliseconds and complete evidence association separately.
It computes requester-local outcomes using privately borrowed expectations and
Timing. The provider never receives expectations or returns an authoritative
pass/fail assertion. Missing resources/evidence/timing produce Inconclusive under
the family's normative algorithm. Custom metadata cannot install these rules.

Agreement contains an owner-authorized selection, challenge, exact quote bytes
and hash, total spending maximum and explicit optional third-party binding.
Invocation is check-specific: roles, offering/definition, original quote, charge,
response milliseconds and local deadline. An included check may have price zero;
no zero-valued ordinary Ledger reservation is created.

Stamp contains a trusted platform clock epoch and milliseconds, never peer time.
Comparable send/receive stamps yield Timing.Within; unavailable or different clock
epochs yield Ambiguous. The response clock begins after the complete challenge is
sent. Counter overflow fails explicitly. Included attempts and all replay bindings
are journaled in the same operation namespace as ordinary work.

Rules.evidence_schema explicitly describes provider evidence separately from the
requester-computed check.output; its conservative default is Unsupported. Known
standard Peer Protocol bindings use their specified evidence fields independently.
Terms freezes mode/entitlement, price and response milliseconds. terms validates
remote eligibility, positive paid checkPrice or valid included scope/limit and
positive response interval. Agreement now includes these quoted Terms; admission
recalculates them once and rejects any drift before a hold or execution. Original
persisted mode, charge and response fields already retain this binding.

Included limits count complete responses rather than all accepted attempts.
Allowance distinguishes Reserved, Consumed, Released and Unknown. Delivery is a
trusted local provider observation: Complete normal response, definitively
Incomplete before terminal transmission, or Unknown possible delivery. Acceptance
reserves a finite slot; acknowledged abort without complete evidence releases it.
Requester receive_check consumes the allowance only after valid complete evidence.
Provider commitment alone keeps it Reserved until the wire owner records delivery.
Unknown observations retain a guard and cannot be silently changed to Incomplete.
These observations are local owner contracts, not new wire receipts.

The mutable private image appends a delivery tag while keeping original six-field
accepted bindings unchanged. Legacy six-field images with known received evidence
restore consumed state; other legacy guards restore Unknown conservatively. New
images preserve all observations across restart. Older binaries may refuse the
extended mutable image; no code resets or overwrites it to permit downgrade.
