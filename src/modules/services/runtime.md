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
Runtime execution, settlement and recovery transitions remain subsequent steps;
admission alone is not successful execution or a complete Services capability.

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
