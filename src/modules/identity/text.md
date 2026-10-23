# Canonical textual IDs

format receives an IdentityId of exactly 32 bytes and returns 58 ASCII bytes:
the exact lowercase prefix cr2se: followed by 52 uppercase RFC 4648 Base32 symbols.
There is no equals padding. Any unused bits of the final symbol are zero.
Formatting has no fallible protocol inputs, I/O, key access or normalization;
it only displays the provided public identifier. Parsing is a separate next step.
