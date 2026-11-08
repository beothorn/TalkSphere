# Invocation role authority

Authorize consumes an immutable Board Selection, trusted authenticated Peer,
installed Registry, optional owned local input and a caller Context. It checks
local identity belongs to requester/provider, peer matches the opposite role,
key requester matches the selection and stream is nonzero. TCP direction is
irrelevant. Local requesters require Request authority and explicitly supplied
input; providers require Invoke authority and an actual provider registration.

Definition and input payload byte grants are checked before retention. Values
also have independent node/depth/byte limits. Registered contract and economic
semantics must match the selected terms. Hints and remote descriptions cannot
create grants, input or code installations. No reservation or execution occurs.

Authorized exposes immutable selection, key, explicit input and pinned exact
implementation. It does not itself confer Ledger authority. These process-local
pins are later copied into durable accepted-operation ownership.
