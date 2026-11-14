# Configured incoming endpoint

`Endpoint` is a cloneable trusted factory for the authenticated stream bridge.
Construction receives explicit real Board, Registry, Services and Ledger owners,
their independent authority issuers, admission/fee policy and finite limits. It
checks configured owner identities and limits, creates bounded quote owners and
trusted clock state, and performs no socket or store initialization. Clones share
the original quote domains, policies and durable owners.

`Config.local` is the Identity-derived receiving node. `authority`, `services`,
`board_authority` and `ledger_authority` belong to their separate modules; wire
messages never supply them. `board` provides current publication/definitions;
`registry` provides installed independent semantics and handlers; `runtime` owns
accepted invocation state; `ledger` owns accounting. `gate` performs read-only
trust/economic admission. `transfer_policy` selects an exact issuer fee. `limits`
bounds Peer Protocol values/records and `service_limits` bounds Services quotes
and state. Receiving modules check every grant against their own authority.

The factory accepts original authenticated incoming scopes for its local identity;
it refuses local opener use. Explicit outgoing request factories are separate.
It serves Board/definition reads, provided work/check proposals, exact acceptance,
input bodies and markers, committed output, explicit family-authorized cached
retries, balance/transfer reads, fee quotes/acceptance and configured claim calls.
Unsupported families/rules and missing protected capabilities return explicit
refusals, never installed code or fabricated success. Wanted publisher policy and
complete outgoing factory wiring remain under development.

Service selection uses the installed definition's contract description and the
current Board, not peer descriptions. Cached Messaging retries consult the actual
retained family/owner before fresh Board/economic admission. Acceptance binds the
original exact quote and reserves before allocating input or executing. Complete
bodies, input.end and requester END gate dispatch. Only owner-acknowledged committed
output becomes a result source; incremental transmission never repeats work.

Known semantic refusals preserve protocol categories. After acceptance, a normal
failure requires the original coordinator to acknowledge uncommitted abort.
Possible committed effects cannot become an uncharged failure. Malformed ordering
is a stream error. Unaccepted quotes are discarded on refusal/closure using narrow
cleanup calls; pre-dispatch accepted holds are released only by acknowledged owner
abort. Dispatched/uncertain work stays in its durable owner after connection loss.

Checks use the original authenticated input.end receipt instant, not the worker's
later execution time. Normal bridge closure proves complete local result/END
transmission; included consumption is recorded by the actual coordinator. Lost
responses remain unknown when END may have been sent. Paid delivery does not enter
the included-entitlement path or invent a refund for upstream issue10.

`Claims.redeem` is an optional protected local port receiving original association,
original deadline and the validated encrypted request. Its implementation must
call the original Ledger claim adapter and Identity decryption authority, preserve
binding/storage failures and return only a bounded public claim result. No grant
or key comes from the peer. Absence is Unsupported; this trait is not an implicit
production decryptor or a bypass for Ledger validation.
