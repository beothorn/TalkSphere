# Decrypt for the local identity

`decrypt(request)` first parses the complete bounded object, then compares its
recipient ID and persistent public key with authorized local values. It borrows the
persistent private capability for agreement, derives the exact content key, and
authenticates the original full header before releasing owned plaintext.

Malformed structure and limits retain their local error categories; mismatched
recipients, non-contributory agreement and bad tags fail authentication. No failure
returns plaintext or a partial object. Shared material and content keys erase on
exit, while the borrowed persistent key remains usable. Plaintext remains untrusted
application data and does not identify a sender or imply forward secrecy.
