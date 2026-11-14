# Protected issuer claim adapter

`Claims.new` receives the original Ledger, its separate authority, a Peer Protocol
authority, a shared protected Decryptor and finite wire limits. It validates limits
without decrypting, issuing credits or opening a stream. Only trusted composition
may construct and install this object as the endpoint's optional Claims capability.
The dependencies retain their own lifetime, storage locks and secret protection.

`redeem` receives the original authenticated association, absolute monotonic
deadline and bounded parsed control from the bridge worker. It issues narrowly
scoped local Invoke and Ledger Control grants, retaining the supplied deadline,
and delegates exact-size, ciphertext authentication, issuer/redeemer binding and
owner-specific replay to the public claim responder and Ledger. Foreign owners,
expired calls, invalid ciphertext, unavailable keys, exhausted bounds and storage
uncertainty remain typed failures. Only public claim ID and original stored amount
leave this boundary; no private key, clear bearer secret or administrator wire
operation is exposed. No callback runs on the Transport reactor.
