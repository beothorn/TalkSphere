# Canonical textual IDs

format receives an IdentityId of exactly 32 bytes and returns 58 ASCII bytes:
the exact lowercase prefix cr2se: followed by 52 uppercase RFC 4648 Base32 symbols.
There is no equals padding. Any unused bits of the final symbol are zero.
Formatting has no fallible protocol inputs, I/O, key access or normalization;
it only displays the provided public identifier. Parsing is available through parse and normalize.

parse accepts exactly 58 bytes, the exact lowercase prefix, and ASCII RFC 4648
Base32 symbols with either letter case. It returns a binary IdentityId or
InvalidInput; whitespace, padding, Unicode lookalikes, invalid symbols and
nonzero final unused bits fail. It never trims or repairs input. normalize
parses then formats valid input, returning canonical uppercase Base32 only.
Both operations are bounded and perform no entropy, storage or network calls.
