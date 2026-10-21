# Binary identity derivation and validation

`derive(crypto, version, algorithm, key)` accepts selectors 1/1 and exactly 32 key
bytes, hashes exactly 01 || 01 || key and returns IdentityId. Unsupported selectors
and wrong lengths fail before hashing; dependency failures propagate. Derivation
is byte hashing and does not validate point encoding or prove possession.

`validate(crypto, version, algorithm, key, claimed)` requires a 32-byte claimed ID,
derives and compares it, then applies canonical non-small-order public-key
validation. It returns PublicIdentity only after both checks. Mismatches and
invalid public keys fail even when hashing alone succeeds. Success causes no
connection, trust, credit or proof-of-possession event. IdentityId equality uses
binary bytes, independent of contact endpoints and display spelling.
