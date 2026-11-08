# Installed contract registry

Registry.new binds one local identity, owner Authority, positive Limits and a
trusted independent Standards catalogue. NoStandards rejects every reserved
cr2se. contract. A downloaded definition never becomes a catalogue entry.

register requires Register authority and a complete known requester definition,
with optional separate provider. It validates reserved standard definitions
against Standards, refuses duplicate exact service/version keys, and enforces
handler capacity. It rechecks authority after acquiring its writer lock.
Registered retains requester/provider implementations; requester returns its codec
and provider returns Unsupported for requester-only entries.

select pins an active exact service/version and optionally requires provider
support. drain requires Register authority and disables new selection while
retaining old implementations. recover requires Recover authority and can retrieve
a drained implementation; absence returns Unavailable to gate accepted recovery.
No version substitution or silent dispatch fallback occurs. These process-local
pins alone do not replace durable coordinator ownership across restart.

The Board Capabilities adapter checks complete logical contract equality,
family-validated economic terms, active availability and actual provider support
for provided offerings. Wanted offerings require requester support; local input
and spending still require invocation authority. Only cr2se.identity is built in
as a precondition and requires trusted authentication. Unknown conditions fail.
Failures stay typed; a conflicting registration maps to stale Board state.

local_identity returns only the configured receiver ID. validate_update checks
registration purpose/domain/bytes without effects, allowing composition to validate
authority before unpublishing. Registry.drain alone changes registry eligibility;
application publication writers use the serialized composition Publication facade
to remove advertised offerings durably before disabling implementations.
