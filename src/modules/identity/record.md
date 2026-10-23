# Local credential record

CredentialRecord is guarded zeroizing staging for one complete local credential,
with no Clone, Debug or general byte-export interface. RECORD_BYTES is 232.
This is an implementation-specific private format, not CR2SE wire encoding or
an interoperable backup standard. Only protected local adapters may retain it.

The version-one layout is eight ASCII bytes TSID0001, a 32-byte Ed25519 seed,
a separate 32-byte X25519 private input, 32-byte ID, 32-byte signing public key,
32-byte encryption public key and 64-byte binding. This record is not encrypted;
platform access controls must protect it.

encode takes already-authorized IdentityKeys and Limits, checks record capacity,
derives the public association and transfers both inputs into zeroizing staging.
from_protected_bytes consumes a protected buffer and checks the budget, exact
length and version before interpreting fields. Too-large records return
ResourceLimit, wrong lengths InvalidInput and unknown versions Unsupported.

The constructor does not claim the contents form a valid identity. decode imports
both keys, derives their public values and rejects any cached ID/key/binding
mismatch with IdentityMismatch, erasing acquired staging on failure. Only then
are protected pairs returned to the authorized caller. No replacement keys are
generated. len returns the fixed staged length; is_empty is always false for a
successfully constructed record. Durability and installation belong to the store.
