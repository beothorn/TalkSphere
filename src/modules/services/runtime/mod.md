# Durable invocation admission

Config supplies local identity, receiving Authority and positive finite Limits.
Coordinator.open explicitly initializes absent state under Register or reopens
versioned complete records under Recover. Recovery requires exact compatible
registered implementations, including retained drained handlers; missing or
changed implementations gate affected work. The coordinator retains its trusted
registry and rejects accepted handles from other implementation instances.

Payment contains only an explicit third-party transfer identity and exact fee;
absence is required for ordinary modes by Ledger. admit retains exact accepted
quote/Board/definition/publisher/direction and economic terms before reserving.
The accepted ceiling includes fees; overflow and excess spending fail. It calls
the real accounting reserve under the original requester/connection/stream key.
Exact retries reuse the association and hold; changed bindings conflict. reserve
recovers only the original hold after a lost local acknowledgement, never work.

status requires Observe and reports only local Phase. Complete-image comparisons
and single-writer serialization guard concurrent journal updates. Possibly committed
writes gate dependent calls. Raw input/output and secrets never enter logs.
Admission is not successful execution; only acknowledged participant delivery permits
a committed result. Focused admission, execution, settlement, recovery, payment
and inspection files implement this lifecycle.

supply_input retains complete input only while Reserved, after the installed
requester validates accepted metadata, schema, content and original byte limit.
Exact local re-delivery reuses the retained value; changed delivery conflicts.
Malformed input leaves the hold uncharged and never executes. Body fragments must
first complete through bounded Body ownership or an equivalent validated adapter.

start durably records Executing before calling the pinned provider with complete
input and explicit public invocation contracts. A repeated Executing call returns
Unknown and does not execute again. Acknowledged dispatch is not validated success;
completion/status recovery must establish the staged result and subsequent decision.

stage queries the original provider's retained completion, independently validates
schema and contextual quote rules, recalculates fixed/metered price and persists
complete output/result/effect bytes before preparing settlement. Metered duration
comes from the typed completion's billedMilliseconds field. Dispatch/staging are
not terminal success. Repeated staging never repeats execution and no caller session
owns the sole copy of a retained durable result.

prepare obtains a durable owner/key/effect-bound handler token and persists it
before preparing the real Ledger charge. Ledger receives a domain-bound SHA256
reference to the coordinator-retained input/result/effect and accepted terms.
Exact retry/status reconstruction recovers participant tokens without repeating
execution. Only after both participants acknowledge preparation does the journal
enter Prepared; partial acknowledgement loss preserves holds and recovery data.

decide durably records exactly one Decision.Commit or Decision.Abort. Commit is
allowed only from Prepared with retained complete output and participant tokens.
Exact decision repetition is idempotent; opposite decisions conflict before any
participant delivery. Abort can resolve uncommitted work but can never replace
Commit/Committed. A decision is independent of peer receipt and payment status.

commit requires a previously recorded Commit. It queries original participant
status, skips already committed effects and delivers only matching prepared
effects. Missing/contradictory participant state is Unknown; it never creates a
replacement obligation after losing history. It enters Committed only after both
participants acknowledge durable commitment. Lost replies retain Commit for
status recovery and decision replay, without execution or duplicate charges.

result requires Observe and returns owned complete output only from Committed;
pending states are Unknown and aborted work conflicts. Admission/new execution
is conservatively gated while any decision awaits delivery. This sacrifices local
availability to avoid successful partial work or conflicting resource observations.

abort delivers only an explicit stored Abort, resolving original prepared handler
and Ledger tokens and releasing uncommitted resources once. A missing Ledger hold
is not recreated merely to abort it. Abort cannot replace Commit/Committed; a
cancelled call alone changes no decision or obligation. Lost participant replies
leave Abort pending for recovery, and no service execution is retried.

Recovery stops and reopens the exclusive coordinator and affected participants
from their authoritative stores under fresh authority. resume then advances only
original durable phases: query staged completion, prepare original effects, record
or deliver the decision, and return the terminal phase. It never calls execute or
reissues a peer invocation. Reserved input awaiting an explicit start remains
Unknown; an unrecoverable external execution remains Unknown. Operators may choose
an authorized abort before a commit decision, never from a participant timeout.

Registry and Coordinator must share the same local Services authority domain;
foreign registration domains are rejected before opening/mutating journal state.
Ledger retains its separate authority and explicit coordinator grant.

Requester-only coordination needs no local provider. outbound durably records the
attempt before returning explicit Invocation/input data to the trusted peer adapter;
a repeated call is Unknown and never resends work. receive accepts only a complete
successful RemoteResult on the authenticated original provider/stream, validates
it through the same retained-result rules and prepares only the local Ledger.
RemoteResult contains output, complete bytes and final price, not a remote private
effect/token. Services generates its own retained-result reference.

A lost remote response remains Unknown. Generic abort cannot erase a dispatched
requester's possible obligation or its retained valid successful result. decline
records an explicit complete original-provider uncommitted failure from the trusted
peer adapter, permitting local hold release. It is never synthesized from timeout,
connection loss or cancellation. Local participant commitment still proves no
atomic cross-peer exchange. Record recovery requires handler tokens only where
that node actually owns a provider participant.

payment reads authoritative Ledger state only after local Committed; missing
accounting history is Unknown. payment_request requires coordinator authority
and returns explicit original issuer transfer/query data for composition.
confirm_payment accepts only Ledger's opaque original-issuer Receipt; Ledger
validates provenance and exact roles/ID/gross/net/fee and applies evidence once.
payment_problem records refusal or delivery uncertainty without changing service
Phase, erasing results or executing again. Issuer networking/signing remains an
explicit later Peer Protocol/Identity composition responsibility.

verify invokes only the installed requester-local verifier with original input,
accepted terms, retained complete output and optionally borrowed local expectations.
It never calls a provider, reserves money, refunds work or changes trust. Missing
successful evidence or unavailable verification dependencies yield Inconclusive.
The requester computes Pass/Fail; provider assertions alone cannot establish it.
Expectations are neither journaled nor transmitted and remain caller-owned.

New paid admission refuses an operation key already present in Ledger without matching Services history; never initialize a missing coordinator journal as a repair for accounting history. Restore and recover the original complete records instead. Revoked authority stops calls without erasing durable decisions.
