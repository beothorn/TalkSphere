# Identity Cryptography adapter

The real Provider implements Identity's Cryptography port through existing public
hash, verification-key and signature operations. Hashing and verification preserve
the provider's input ceiling; public-key validation requires the exact canonical
non-small-order encoding. No access to Cryptography's private key state is needed.

Conversion from CryptoError preserves invalid input, denied authority, unavailable
dependency and resource limits; authentication failure becomes Identity InvalidProof.
No failure adds secrets, retries, trust or connection state. The implementation
supports trusted in-process composition and grants no remote signing authority.
