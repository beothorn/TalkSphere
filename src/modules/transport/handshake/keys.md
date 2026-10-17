# Directional material

DirectionKeys owns an opaque key and 16-byte nonce_prefix. SessionKeys owns
send and receive DirectionKeys, with no cloning or diagnostic exposure of keys.

derive_keys consumes an ephemeral private capability, remote ephemeral public
key, role and transcript through Crypto: agree; hash the KEYS domain separator
and transcript for salt; HKDF extract; expand both key labels and both prefix
labels. It returns I2A as send for the initiator and as receive for the acceptor.
Every primitive failure propagates and locally owned secret handles are dropped.
The Crypto adapter must reject an all-zero shared secret and erase keys on Drop.

nonce concatenates the supplied 16-byte prefix with an unsigned 64-bit sequence
in big-endian order. Sequence lifetime, replay protection and wrap prevention
belong to the integrity driver, not this byte construction.
