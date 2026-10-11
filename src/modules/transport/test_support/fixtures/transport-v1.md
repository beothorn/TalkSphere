# Transport version 1 test vectors

These deterministic values test the handshake, identity derivation, session-key
derivation, and first protected frame defined by [Transport specification](../README.md).
All byte strings are lowercase hexadecimal. Private values here are test data
only and must never be used by a real identity or connection.

## 1. Fixed inputs

```text
initiator Ed25519 seed:
000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f

acceptor Ed25519 seed:
202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f

initiator raw X25519 private-key input:
404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f

acceptor raw X25519 private-key input:
606162636465666768696a6b6c6d6e6f707172737475767778797a7b7c7d7e7f

initiator HELLO nonce:
808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f

acceptor HELLO nonce:
a0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7b8b9babbbcbdbebf
```

The standard X25519 operation performs its required scalar clamping internally.

## 2. Public keys and CR2SE IDs

```text
initiator Ed25519 public key:
03a107bff3ce10be1d70dd18e74bc09967e4d6309ba50d5f1ddc8664125531b8

acceptor Ed25519 public key:
29acbae141bccaf0b22e1a94d34d0bc7361e526d0bfe12c89794bc9322966dd7

initiator ephemeral X25519 public key:
79a631eede1bf9c98f12032cdeadd0e7a079398fc786b88cc846ec89af85a51a

acceptor ephemeral X25519 public key:
675dd574ed7789310b3d2e7681f3790b466c773b1521fecf36577958371ea52f

initiator CR2SE ID:
d948a9a847999305be609db39d9ca50b7c93184ec36bdf080f6f46415e19925d

acceptor CR2SE ID:
774504edf47bae28a4f1154bf4516ae74f090ee5b032464586d0c21d24d9b249
```

Each CR2SE ID is `SHA-256(0x01 || 0x01 || ed25519PublicKey)`.

## 3. HELLO payloads

Both payloads are exactly 100 bytes.

```text
initiator HELLO:
01010103a107bff3ce10be1d70dd18e74bc09967e4d6309ba50d5f1ddc8664125531b80179a631eede1bf9c98f12032cdeadd0e7a079398fc786b88cc846ec89af85a51a808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f

acceptor HELLO:
01010129acbae141bccaf0b22e1a94d34d0bc7361e526d0bfe12c89794bc9322966dd701675dd574ed7789310b3d2e7681f3790b466c773b1521fecf36577958371ea52fa0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7b8b9babbbcbdbebf
```

For an additional transcript check:

```text
SHA-256(helloTranscript):
b8179c941632a1945354cafc2eb7b9e892239e2165efdd1c770898e8fa0cbd39
```

## 4. AUTH signatures

```text
initiator AUTH payload:
ee7fd684c9ffe4be9690028ae6cc95fab3e08a3d0f1c1e011125bfe0f7519360b9f361d495d9d3f76c8b6afd8e9e681f3f7726d02c26bebd952ca62f196b7904

acceptor AUTH payload:
bdfdbe34c927de4efd3e6625f4a58f95f78fa2091dec580aaf52a22e9a39dfe7e46d8cbf83752608d1d4246da4c5b9d68df5d47104ba9ac7a1e58269c1dada01
```

These signatures use the role-specific `authMessage` defined by Transport specification.

## 5. Shared and derived material

```text
X25519 shared secret:
d6fb939511b2381bc8599b4b8edc5968829450dfd7a87aebe78a703cd04cd54e

HKDF salt:
8aefb9765055e4bd0df062bdfe80308fc8798c984e8de4349fad5ea4012b9bb2

HKDF extract PRK:
02c069957c9eb792641672c5f6ab502061c24d2532f442e2c5ff44246f5e8827

initiator-to-acceptor key:
c08a898e59badb2806465fa3fe2efcf16ede0f6f55212413a0f583f991d54225

acceptor-to-initiator key:
f94e68d81bafb537b6b910aae2e65a2b3dc747244309de4ba3d9c1200045a892

initiator-to-acceptor nonce prefix:
812a8007edee5869f7312364c75b45d1

acceptor-to-initiator nonce prefix:
7f474199ecdeb802f8b97a725916ece8
```

## 6. First protected frame

The initiator sends a `PING` on stream `0`. This is its first protected frame,
so the implicit initiator-to-acceptor sequence number is `0`.

```text
header:
010300000000000000000008

payload:
0001020304050607

nonce prefix || uint64_be(0):
812a8007edee5869f7312364c75b45d10000000000000000

integrity tag:
2d72a6c440a960b2a6282450f9fb7ac1

complete bytes written to TCP:
01030000000000000000000800010203040506072d72a6c440a960b2a6282450f9fb7ac1
```

The tag is the XChaCha20-Poly1305 result for empty plaintext with
`header || payload` as authenticated data. Changing any header byte, payload
byte, key byte, nonce-prefix byte, or sequence number must make verification
fail.

The acceptor answers with its first protected frame, a `PONG` using its own
directional sequence `0`:

```text
header:
010400000000000000000008

payload:
0001020304050607

nonce prefix || uint64_be(0):
7f474199ecdeb802f8b97a725916ece80000000000000000

integrity tag:
5feb9216750a6618af80f39b7f813ea6

complete bytes written to TCP:
01040000000000000000000800010203040506075feb9216750a6618af80f39b7f813ea6
```
