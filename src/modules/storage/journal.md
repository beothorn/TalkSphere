# Durable participant staging and preparation

Journal is a serialized family-owned journal over the Storage Persistence and
Crypto ports. Local family/Services adapters supply trusted original operation
keys; this is not a peer endpoint or remote replay permission. The platform
adapter must hold exclusive writer ownership before construction.

initialize explicitly creates an absent image with provider identity and initial
retention Anchor, rejecting existing state with Conflict. open requires an
existing valid image for that provider and never resets corruption or absence.
Byte verification and coordinator recovery must precede advertisement/new work.

stage takes an original OperationKey, digest binding exact accepted input/terms,
and complete Completion. It durably retains output, bytes, proposed effect and
price. Identical local repeats return the retained completion without execution;
changed input/result/effect/price conflicts. Aborted work cannot become staged
success. completion retrieves original complete work without a new reservation.

prepare takes the original key and exact immutable effect. It creates and
persists an opaque token bound to provider, key, accepted binding, effect and fresh
injected entropy before acknowledging preparation. Repeated preparation returns
the original token, including after commitment. Wrong effects or aborted work
conflict; missing operations are Unavailable. Crypto failure causes no preparation.

status observes absent, family-local staged, prepared or terminal phases. It
neither chooses a coordinator decision nor interprets timeout as abort.

An uncertain snapshot write closes the in-memory gate: further stage, completion,
prepare and status return Unknown. reload reads only installed valid metadata and
reopens the gate; absent/corrupt/unavailable metadata keeps it closed. Known
precommit failures preserve the last acknowledged image. Snapshot bounds and
logical result budgets are enforced before an acknowledged mutation. No method
executes work, opens peer streams, changes balances or grants remote retries.
