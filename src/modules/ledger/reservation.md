# Ledger reservations

Reserve takes Coordinator authority, OperationKey, immutable Terms and trusted
local Policy. The key requester must match Terms. An exact retry returns the same
opaque local/key association; conflicting terms fail without another hold.
Provider mode reserves requester-owned provider credits or provider-issued credits
recognized for the requester according to this node's role. Balances stay unchanged;
Available subtracts every Reserved or Prepared hold from the selected balance.

Admission is bounded and serialized with all accounting mutations. The complete
binding and hold are durable before success. Reservation is not authority and has
no peer encoding. Available requires Read, rejects self-balances and gates unknown
state. The private journal has strict finite arrays, exact integer/byte fields,
unique keys and canonical re-encoding. Unsupported modes fail explicitly until
implemented by their own plan steps. Status distinguishes reserved, prepared,
committed and aborted; later steps implement decisions and status lookup.

Requester mode holds finite issuance on the requester and incoming owned-balance
headroom on the provider. Requester issuance also pins destination issued-balance
headroom. Administrative grants/reconciliation must respect these holds; no owned
self-currency is required. Explicit requester-credit acceptance is provider policy.

Internal mode reserves a separate finite local budget. Explicit Coordinator authority
and resource capacity authorize self work; no remote trust assertion or self-balance
is created. The same coordinated identity writer must own this budget across nodes.
