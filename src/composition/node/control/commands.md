# Control commands

Accounting (local Ledger only): `grant` issues finite credits once per request id (a resend with the
same parameters repeats the reply; reuse with other parameters is refused; memory spans the node's
lifetime); `own` and `trust` set absolute values, so repeating them is safe; `balance` shows the
issuer-separated view for one peer.

Inspection: `status` (identity, address, role, leases, unknown outcomes, live connection count and connected peer IDs, balances at known
providers), `leases` and `outcomes` (optionally unknown only).

Paid Storage, always under an explicit spending ceiling:
- `fetch` authenticates a contact and returns its provided offerings. A contact may be an ID alone: the request then uses
  the peer's live authenticated connection, whichever side opened it, and fails at once without charge when there is none.
- `store` sends a nonempty object (at most 16384 bytes); the lease is cataloged only after the result
  is validated.
- `retrieve`, `renew`, `remove` and `check` use the cataloged lease and provider. These have exact
  fixed prices that the provider requires as the quote maximum, so the node computes the price from the
  selected terms and refuses when it exceeds the ceiling. Retrieved bytes are returned only after
  validation (a full retrieval also rechecks the whole-object hash). Renewal defaults its expected
  expiry to the cataloged one. A check keeps its expected byte local.

An exchange that was opened and then lost is reported as `unknown_outcome`, recorded in the journal and
never retried or replaced by another store. A local spending refusal at the quote is raised before
acceptance and is reported as `insufficient`. Only Storage operations exist; other optional families
return `unsupported`.
