# Encrypted owner-bound claim redemption

Portable.encrypt builds exactly ASCII CR2SE-CLAIM-REDEEM-V1 followed by issuer32,
owner32, claimId32 and secret32, with no amount/terminator. The recipient credential
must match the issuer and pass existing generic envelope binding validation. Plaintext
is guarded; encryption uses fresh ephemeral material/nonce through Cryptography.

Decryptor is an explicitly trusted protected dependency. IdentityDecryptor borrows
an existing Identity handle and scoped Decrypt capability. Redemption.decrypt requires
Control authority, the exact fixed envelope length and authenticated nonlocal owner.
After full decryption authentication it checks exact plaintext length/domain/local
issuer/owner before constructing guarded Redemption. Public observations reveal only
claimId, owner and SHA256 commitment, never the bearer secret. All caller/source
buffers remain the adapter's cleanup responsibility. Transport integrity alone is
insufficient confidentiality. No plaintext is released provisionally or logged.

ENVELOPE_BYTES is exactly 297 (149 plaintext plus the generic148-byte overhead).
The fixed size is checked before decryption; no claimed portable amount is included.
