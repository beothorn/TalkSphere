# Hosted Publications placement

A placement is one host's accepted agreement to retain one signed publication. States: `pending -> removed | expired`; there is no recovered
state because reading does not consume a publication. Terminal states never move; only pending is active, and a placement is readable while
`now < expiresAt` (the expiry instant itself is expired).

`Record`: placement ID, the signed `Metadata`, the publisher's public key and signature, `acceptedAt`, `expiresAt`, state, `payments` (the sum
of the store and every renewal price, the ceiling of any release credit) and the recorded `Removal` (removal ID, release credits, price) once
removed, kept so a retry returns it identically. `Placement` adds the exact content bytes (ciphertext when restricted); `publication()`
reassembles the complete signed publication exactly as stored. Content is never printed.
