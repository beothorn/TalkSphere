# Encrypted claim controls

Ports supplies the real local Ledger, separate Control context and trusted complete
Decryptor, normally Ledger's protected IdentityDecryptor adapter. respond requires
receiving Invoke permission and the authenticated nonlocal redeemer's original
association. It accepts only ledger.claim.redeem and checks the exact297-byte
generic envelope bound before decryption/allocation. No peer chooses key handles,
administrator authority, clear bearer fields or issuance amount.

Ledger/Identity authenticate the entire ciphertext, issuer, redeemer, claim ID and
secret before any issuance. Decryption failure, wrong owner/restriction/secret,
unavailable keys, malformed/oversized objects and storage uncertainty cannot produce
successful output. Only public original claimId and stored authoritative positive
amount are returned. Original-owner retries return the same result without issuing
again; another owner cannot reuse it. Transport integrity does not replace encrypted
object authentication. Do not log ciphertext controls, plaintext or bearer material.

Request.new receives already issuer-encrypted bytes, public expected ID and explicit
local Request permission. It retains only ciphertext and original route/deadline;
portable amount hints never enter the control. bytes borrows the encrypted request.
finish requires separate incoming permission, same authenticated issuer/stream,
normal completion, matching ID and positive canonical amount before returning
Redeemed(issuer, id, amount). This observation never automatically overwrites local
owned balances. Explicit failure and abnormal completion cannot publish success.
Claim redemption may commit on its first request, so interrupted exchanges remain
Unknown even without a fee-acceptance phase. No claim creation/grant/trust operation
is exposed to peers.
