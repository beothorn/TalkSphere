# Persistent encryption credentials

`parse(identity_id, signing_public_key, encryption_public_key, binding_signature)`
copies exactly 32/32/32/64 bytes into an untrusted RecipientCredential or returns
`InvalidInput`. `identity_id(public_key)` implements the shared version-1 digest
formula, requiring 32 bytes but not proving possession or trust.

`binding_bytes(public_key)` returns exactly `CR2SE-ENCRYPTION-KEY-V1` in ASCII
followed by 32 key bytes, with no zero terminator or length field. `BINDING_BYTES`
is that fixed size. `bind(signing_key, public_key)` signs those bytes using local
authority. Wrong sizes fail before signing.

`validate(credential)` compares the derived ID before verifying the binding and
returns a ValidatedRecipient only on success. Its `identity_id()` and
`encryption_public_key()` return verified public values. Mismatch or signature
failure returns a structured error; the signature module's strict-policy caveat
applies. Validation does not choose among conflicting persistent keys for one ID,
rotate credentials, assign trust, or prove that an encrypted object has a sender.
Those persistent/cache policies remain Identity's responsibility.
