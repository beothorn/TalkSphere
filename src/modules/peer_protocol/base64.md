# Canonical byte representation

Encode maps caller-owned bytes to the RFC4648 standard alphabet with required
padding, no whitespace and zero unused bits. Decode accepts a string and maximum
decoded length, checks predicted size before allocation, then rejects alternate
alphabet, misplaced/missing padding, whitespace and nonzero unused bits. Empty
bytes encode as empty text. Capacity failure is Limit; malformed spelling Invalid.
Control identifier fields additionally check exact byte lengths. Inline service
bytes impose a separate4096-byte maximum; this shared helper alone does not select
inline versus detached transfer or authorize interpretation of decoded bytes.
