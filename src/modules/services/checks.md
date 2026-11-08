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
