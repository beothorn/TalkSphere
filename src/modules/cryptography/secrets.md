# Protected secret ownership

`Secret<P>` owns 32 protected bytes tagged with one purpose: `Signing` (Ed25519
seed), `Persistent` or `Ephemeral` (X25519 input), `Shared` (agreement result),
`Prk` (HKDF extract), `Content` (encryption key), or `Integrity` (Transport key).
The purposes are distinct types; capabilities expose no public byte accessor,
cloning, copying or diagnostic formatting.

`new(bytes)` takes an already zeroizing fixed-size buffer from an authorized local
caller. It cannot fail for length because the buffer is exactly 32 bytes; this
constructor does not validate algorithm inputs or prove external authorization.
Application composition must not expose secret import to peers. On drop the owned
bytes are erased using the established zeroize library; staging buffers also erase
on drop. This cannot promise erasure of caller copies, compiler temporaries,
registers, swapped pages or persistent backing stores. Identity owns persistence.

The production dependency uses zeroize's portable volatile-write/compiler-fence
mechanism. Cargo.lock pins the resolved version; no custom unsafe erasure is added.
Tests observe lifetime release, not inaccessible freed memory.
