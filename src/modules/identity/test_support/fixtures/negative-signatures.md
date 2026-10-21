# Cryptography acceptance tests

[Specification](../README.md) · [Inputs](../inputs/README.md) · [Outputs](../outputs/README.md)

Use the [shared harness and common cases](../../../AcceptanceTests.md). Required
suite; no other running module is needed. `V` is the existing
[fixed envelope vector](../pseudocode/README.md#19-fixed-identity-encryption-test-vector).
Compare stored vector bytes, not values produced by the encoder under test.

```text
TEST CR-01 known hashes and chunk independence
  ACT SHA256(empty); ASSERT hex(result)="e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
  FOR EACH partition of ASCII("abc") including single-byte delivery:
    ACT hash ordered chunks
    ASSERT hex(result)="ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"

TEST CR-02 exact ordinary Ed25519
  ACT sign fixed Transport AUTH vector bytes with its fixed seed
  ASSERT signature equals pinned AUTH signature; verification=true
  FOR EACH [message bit flip, signature bit flip, wrong public key, wrong domain]:
    ACT verify(mutated fixture); ASSERT false
  ASSERT Ed25519ph or native object serialization is not used
  FOR EACH row IN the negative signature vectors below:
    ACT verify(row.A, row.message, row.signature); ASSERT false
    ACT verify batch containing row and valid Transport AUTH vector; ASSERT rejected
    ACT verify each individually; ASSERT same per-signature decisions
  ASSERT valid existing binding and AUTH vectors still pass
  ASSERT verification uses the uncofactored equation and canonical encodings

TEST CR-03 persistent key binding
  ACT bind/verify independently generated persistent X25519 public key
  ASSERT signed bytes = ASCII("CR2SE-ENCRYPTION-KEY-V1") || publicKey
  ASSERT no zero terminator or length prefix added
  FOR EACH [claimed ID mismatch, key substitution, signature length 63/65]:
    ACT validateCredential; ASSERT REJECT before encrypting

TEST CR-04 full envelope vector
  ACT encrypt V.plaintext using V.recipient, fixed ephemeral key and nonce
  ASSERT kdfContext, salt, PRK, key, header, sealed bytes and full object = V.expected
  ACT decrypt V.object with V.recipient key
  ASSERT plaintext = V.plaintext; no extra external associated-data parameter

TEST CR-05 self and recipient encryption
  FOR EACH recipient IN [self, another identity]:
    ACT encrypt then decrypt empty plaintext and UTF8("hello €")
    ASSERT returned plaintext equals exact original bytes
    ASSERT header length=132; sealed length=plaintext length+16
    ASSERT entire object length=148+plaintext length
    ASSERT header recipient ID/key match intended recipient

TEST CR-06 authenticated header and ciphertext
  FOR EACH byte position IN valid header, ciphertext and tag:
    ACT decrypt object with one bit flipped at position
    ASSERT REJECT; ASSERT released plaintext byte count=0
  ACT decrypt valid object using wrong identity or persistent key
  ASSERT REJECT; ASSERT released plaintext byte count=0

TEST CR-07 format and length boundaries
  FOR EACH [unsupported format/agreement/KDF/AEAD selector, truncated header,
            sealedLength<16, sealedLength=U64_MAX exceeding input/budget,
            fewer sealed bytes, one trailing byte]:
    ACT decrypt malformed object
    ASSERT REJECT before excessive allocation; ASSERT no plaintext
  ACT stream ciphertext without final tag
  ASSERT no provisional plaintext is released

TEST CR-08 key agreement failures
  FOR EACH [invalid peer X25519 key, primitive returns 32 zero bytes, primitive error]:
    ACT derive envelope or Transport secret
    ASSERT REJECT; ASSERT raw shared result never used directly as encryption key

TEST CR-09 fresh object material and cleanup
  supply two distinct fixture ephemeral pairs/nonces
  ACT encrypt identical plaintext twice for same recipient
  ASSERT each call consumes fresh material; headers and ciphertexts differ
  ASSERT both decrypt correctly; no nonce/key pair reused
  ACT success, failure and cancellation with instrumented protected-secret port
  ASSERT transient key material is released/erased when no longer needed
  ASSERT diagnostics/public results contain none of the fixture secrets
  FOR EACH fault IN [second persistent pair generation, public-key encoding,
                     identity derivation, binding signing, atomic key storage,
                     ephemeral public-key encoding, nonce randomness after ephemeral generation,
                     X25519 error or wrong-length result, HKDF extraction,
                     HKDF expansion after PRK acquisition, wrong-length expanded key,
                     header construction, AEAD encryption/decryption]:
    ACT inject failure, then cancellation, at that boundary with acquired secrets recorded
    ASSERT every acquired staging secret erased; no unowned returned secret
    ASSERT primitive-internal partial secrets erased even if no value was returned
    ASSERT no partial persistent identity committed on generation/storage failure
  ACT successful key generation and successful derivation
  ASSERT staging erased; committed handles remain usable
  ASSERT returned shared/derived secrets owned by caller until erased on its exit

TEST CR-10 construction separation
  ACT parse Messaging-specific envelope as generic 132-byte-header object
  ASSERT it is not accepted as the generic format
  ACT encrypt generic object, store opaque bytes, retrieve and decrypt
  ASSERT sender authenticity is not inferred merely from valid decryption
  ASSERT Transport frame integrity alone is not reported as payload confidentiality

TEST CR-11 independent protected units
  ACT decrypt two valid individually authenticated chunks in changed order
  ASSERT primitive authentication alone does not claim original sequence integrity
  ACT validate an application manifest binding positions, lengths and hashes
  ASSERT reordered/missing chunk fails that manifest's verification
  ASSERT no new CR2SE chunk wire format is inferred from this application test
```

## Negative Ed25519 vectors

Hex strings below encode exact bytes. `||` is concatenation; `zero(n)` is n
zero bytes. `V.A`, `V.R`, `V.S`, and `V.message` are the public key, first and
last 32 signature bytes, and message from the pinned valid Transport AUTH vector.
Every row must be rejected. Use both ASCII("first") and ASCII("different") for
rows marked `either`; the identity-point forgery fails for every message.

```text
I = hex("0100000000000000000000000000000000000000000000000000000000000000")
T = hex("ecffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f")
Yp = hex("edffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f")
IbadSign = hex("0100000000000000000000000000000000000000000000000000000000000080")
Lbytes = hex("edd3f55c1a631258d69cf7a2def9de1400000000000000000000000000000010")

A           signature                   message       reason
I           I || zero(32)               either        small-order A and R
T           V.R || V.S                  V.message     order-two A
V.A         I || V.S                    V.message     small-order R
V.A         T || V.S                    V.message     order-two R
Yp          V.R || V.S                  V.message     noncanonical y = p
V.A         Yp || V.S                   V.message     noncanonical R
IbadSign    V.R || V.S                  V.message     x = 0 with sign bit 1
V.A         IbadSign || V.S             V.message     invalid R sign bit
V.A         V.R || Lbytes               V.message     S = L, out of range
V.A         V.R || littleEndian32(V.S + L) V.message  scalar malleability
```

In the final row interpret V.S as an unsigned little-endian integer before
addition. For each invalid A, public identity validation must also reject even
when the claimed ID is correctly derived from those bytes. No normalization or
scalar reduction may turn a rejected row into accepted input.
