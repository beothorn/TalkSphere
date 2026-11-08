# Transfer proof authority

TransferSigner supplies scoped signing over exactly121 bytes. IdentitySigner
borrows a loaded protected Identity provider/handle and Sign capability; it never
extracts a private key. Create requires Ledger Spend authority before entropy or
signing, obtains a fresh16-byte cryptographic transfer ID and validates the resulting
order/signature/key association. Failure returns no authorized payment object.

Verify requires authenticated caller=order owner and local identity=order issuer,
checks the key-derived identity and applies existing strict CR2SE Ed25519 verification
to exact order bytes. A valid proof establishes no issuer credit policy or trust.
Composition supplies both entropy and signer; deterministic substitutes exist only
in tests. No function opens a connection or changes a balance.

Create groups issuer, new owner, gross amount and local public key in an explicit
local request. Local owner comes from the authorized call, not a peer-provided field.
