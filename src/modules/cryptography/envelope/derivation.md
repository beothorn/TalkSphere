# Exact envelope derivation

`context(header)` returns the first 124 canonical header bytes: four suite
selectors, recipient ID, persistent public key, ephemeral public key and nonce.
It propagates invalid header metadata. `salt(context)` requires suite 01 01 01 01
and hashes ASCII("CR2SE-IDENTITY-ENCRYPTION-SALT-V1") plus an explicit zero byte
plus context. Unsupported suite values fail with `InvalidInput`.

`derive(shared, context)` consumes the protected shared result, extracts with that
salt, and expands 32 Content-key bytes using ASCII("CR2SE-IDENTITY-ENCRYPTION-KEY-V1")
plus explicit zero plus context. Shared material and PRK erase on every exit;
returned key ownership passes to the caller. No alternate domains, implicit text
terminators or field orderings are accepted. Tests pin every intermediate and
check that every supported context-byte change separates the derived key.
