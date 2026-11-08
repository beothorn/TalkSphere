# Ledger module boundary

Ledger exports exact accounting/trust values, trusted local authority, versioned
contexts/operation keys, atomic persistence and the serialized state owner. Focused
contracts cover accepted terms/reservations, prepare/commit/abort/status, canonical
transfers, protected signing, exact fee negotiation, transfer replay/query,
original-issuer payment evidence, guarded claims, encrypted redemption, maintenance
and read-only status summaries. See the same-stem companion beside each source.

Error distinguishes Invalid, Unsupported, Denied, Limit, Cancelled, Conflict,
Insufficient, Storage, Unknown and Unavailable. Storage means known precommit
failure; Unknown may mean commitment and never implies rollback. Unavailable
preserves entropy/protected-dependency loss separately from proof denial. Conversion
from Identity/Cryptography preserves bounds, unsupported, conflict and unknown
categories without including private material or peer text in diagnostics.

The application exposes offline read-only ledger-show only. Services coordination,
family effects and peer network records remain their own module responsibilities.
This direct-call/Linux implementation does not complete a useful network node.
