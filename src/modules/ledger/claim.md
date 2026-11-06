# Guarded portable credit claim

Portable owns issuer identity, public claimId, guarded secret32 and positive amount
hint. Parse accepts only cr2se-claim: and canonical RFC4648 encoding of exactly105
bytes: version1, issuer32, claimId32, secret32, uint64be amount. This length is divisible
by three, so canonical padded Base64 has140 symbols and no trailing equals signs.
Wrong prefix, alphabet, length, version, padding or zero amount fails without secrets
in diagnostics. The caller must protect/erase its original imported input too.

Export explicitly returns a guarded secret string for a private channel, never
Debug/Display or CLI logging. Issuer/id/amount getters return public metadata only.
Portable amount is a hint; the issuer's stored amount is authoritative on redemption.
No secret is an identity key or a public contact record.
