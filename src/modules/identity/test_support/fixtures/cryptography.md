# Cryptography pseudocode

This document translates the normative cryptographic contract in
[Cryptography specification](../README.md) into implementation-neutral pseudocode.
The specification remains authoritative. Names such as `SecureKeyStore`,
`NonceAllocator`, byte buffers, and cryptographic operations describe
responsibilities, not required classes, storage formats, or APIs.

The pseudocode assumes conventional, constant-time implementations of Ed25519,
X25519, SHA-256, HKDF-SHA-256, and XChaCha20-Poly1305. Received keys,
signatures, headers, lengths, ciphertext, authenticated data, and decrypted
plaintext are untrusted input.

Every secret-producing call executes inside an already active cleanup guard.
`if assigned` means erase only an acquired secret, never an unassigned value or
error sentinel. Erasure is non-throwing and covers temporary copies; primitives
must erase internal partial results if they fail before returning. Returning a
secret transfers ownership to the caller without an unguarded interval. Scoped
secret owners with equivalent failure and cancellation behavior are acceptable.
Successful atomic key storage retains protected persistent copies; failure
retains neither pair. Erasing staging copies does not erase committed handles.

---

## 1. Constants and data structures

```text
IDENTITY_VERSION                    = 0x01
IDENTITY_KEY_ALGORITHM_ED25519      = 0x01

ENCRYPTION_FORMAT_VERSION           = 0x01
KEY_AGREEMENT_X25519                = 0x01
KEY_DERIVATION_HKDF_SHA256          = 0x01
AEAD_XCHACHA20_POLY1305             = 0x01

ED25519_PUBLIC_KEY_SIZE             = 32
ED25519_SIGNATURE_SIZE              = 64
X25519_PUBLIC_KEY_SIZE              = 32
X25519_SHARED_SECRET_SIZE           = 32
CR2SE_ID_SIZE                       = 32
SHA256_SIZE                         = 32
ENCRYPTION_KEY_SIZE                 = 32
XCHACHA_NONCE_SIZE                  = 24
POLY1305_TAG_SIZE                   = 16

ENCRYPTED_OBJECT_HEADER_SIZE        = 132
UINT64_MAX                          = 18_446_744_073_709_551_615

BINDING_PREFIX = ASCII("CR2SE-ENCRYPTION-KEY-V1")
KDF_SALT_PREFIX = ASCII("CR2SE-IDENTITY-ENCRYPTION-SALT-V1")
KDF_INFO_PREFIX = ASCII("CR2SE-IDENTITY-ENCRYPTION-KEY-V1")

EncryptionPublicCredential:
    identityId                      32-byte CR2SE ID
    ed25519PublicKey                32 bytes
    x25519PublicKey                 32 bytes
    bindingSignature                64 bytes

LocalEncryptionIdentity:
    identityId                      32-byte CR2SE ID
    encryptionPublicKey             32-byte persistent X25519 public key
    encryptionPrivateKeyHandle      protected persistent X25519 key handle

EncryptionHeader:
    formatVersion                   uint8
    keyAgreementAlgorithm           uint8
    keyDerivationAlgorithm          uint8
    aeadAlgorithm                   uint8
    recipientId                     32 bytes
    recipientX25519PublicKey        32 bytes
    ephemeralX25519PublicKey        32 bytes
    nonce                           24 bytes
    sealedLength                    uint64
    encoded                         exact 132 bytes

ParsedEncryptedObject:
    header                          EncryptionHeader
    sealed                          ciphertext || 16-byte tag

EncryptionResourceLimits:
    maximumEncryptedObjectBytes     positive implementation limit
    maximumPlaintextBytes           positive implementation limit
```

The four algorithm identifiers form one version 1 suite. Unknown versions or
algorithm identifiers must not be silently interpreted as this suite.

---

## 2. Checked byte and length helpers

```text
function requireExactLength(bytes, expected, name):
    if length(bytes) != expected:
        raise InvalidCryptographicValue(name + " has the wrong length")
    return bytes


function checkedAdd(left, right, maximum):
    if left < 0 or right < 0 or left > maximum - right:
        raise LengthOverflow
    return left + right


function sealedLengthForPlaintext(plaintextLength):
    if plaintextLength < 0:
        raise InvalidLength

    sealedLength = checkedAdd(
        plaintextLength,
        POLY1305_TAG_SIZE,
        UINT64_MAX
    )
    return sealedLength
```

Parsers validate declared lengths and checked totals before allocation or body
reads. A local cryptographic API may have a smaller maximum input size; values
above that bound are rejected before calling it.

---

## 3. Identity encryption-key generation and binding

Identity generation creates an Ed25519 signing pair and a persistent X25519
encryption pair. The CR2SE ID derives only from Ed25519; the signature below
binds the encryption public key to that ID-defining key.

```text
function makeEncryptionBindingBytes(x25519PublicKey):
    requireExactLength(
        x25519PublicKey,
        X25519_PUBLIC_KEY_SIZE,
        "X25519 public key"
    )
    return BINDING_PREFIX || x25519PublicKey


function bindEncryptionPublicKey(ed25519PrivateKeyHandle, x25519PublicKey):
    message = makeEncryptionBindingBytes(x25519PublicKey)
    signature = Ed25519.sign(ed25519PrivateKeyHandle, message)
    assert length(signature) == ED25519_SIGNATURE_SIZE
    return signature


function generateCompleteIdentityKeyMaterial(secureKeyStore):
    try:
        signingPair = Ed25519.generateKeyPair(CSPRNG)
        encryptionPair = X25519.generateKeyPair(CSPRNG)

        edPublic = Ed25519.canonicalPublicKeyBytes(signingPair.publicKey)
        xPublic = X25519.canonicalPublicKeyBytes(encryptionPair.publicKey)
        requireExactLength(edPublic, ED25519_PUBLIC_KEY_SIZE, "Ed25519 public key")
        requireExactLength(xPublic, X25519_PUBLIC_KEY_SIZE, "X25519 public key")

        identityId = Identity.deriveVersion1Id(edPublic)
        binding = Ed25519.sign(
            signingPair.privateKey,
            makeEncryptionBindingBytes(xPublic)
        )

        handles = secureKeyStore.storeIdentityKeyPairsAtomically(
            signingPair.privateKey,
            encryptionPair.privateKey
        )
    finally:
        securelyEraseTemporaryCopies(signingPair.privateKey if assigned)
        securelyEraseTemporaryCopies(encryptionPair.privateKey if assigned)

    if handles is ERROR:
        raise IdentityStorageFailure

    return {
        identityId: identityId,
        ed25519PublicKey: edPublic,
        x25519PublicKey: xPublic,
        bindingSignature: binding,
        ed25519PrivateKeyHandle: handles.signing,
        encryptionPrivateKeyHandle: handles.encryption
    }
```

Every node operating as this identity uses the same two persistent pairs. Secure
replication among nodes and devices is implementation-specific and must not
transmit private keys as part of ordinary CR2SE protocol operation.

---

## 4. Encryption-key credential validation

```text
function validateEncryptionPublicCredential(credential):
    requireExactLength(
        credential.identityId, CR2SE_ID_SIZE, "CR2SE ID")
    requireExactLength(
        credential.ed25519PublicKey,
        ED25519_PUBLIC_KEY_SIZE,
        "Ed25519 public key")
    requireExactLength(
        credential.x25519PublicKey,
        X25519_PUBLIC_KEY_SIZE,
        "X25519 public key")
    requireExactLength(
        credential.bindingSignature,
        ED25519_SIGNATURE_SIZE,
        "binding signature")

    derivedId = Identity.deriveVersion1Id(credential.ed25519PublicKey)
    if derivedId != credential.identityId:
        raise IdentityMismatch

    message = makeEncryptionBindingBytes(credential.x25519PublicKey)
    if not verifyExactBytes(
        credential.ed25519PublicKey,
        message,
        credential.bindingSignature
    ):
        raise InvalidEncryptionKeyBinding

    return {
        identityId: derivedId,
        x25519PublicKey: credential.x25519PublicKey
    }
```

The binding proves only that the identity signing key authorized the X25519
public key. It does not prove that ciphertext came from a particular sender or
that the identity is trustworthy.

Version 1 defines one persistent encryption pair per identity and no rotation
or multiple-key selection protocol. A recipient credential cache must not
silently choose among different validly signed X25519 keys for the same ID.

---

## 5. General signing and identity-proof boundary

```text
function signExactBytes(ed25519PrivateKeyHandle, protocolDefinedBytes):
    return Ed25519.sign(ed25519PrivateKeyHandle, protocolDefinedBytes)


function parseVerificationPublicKey(encoded):
    // Canonical decoding includes y < p and rejecting x=0 with sign bit 1.
    if length(encoded) != 32:
        return ERROR
    point = Edwards25519.decodeCanonical(encoded)
    if point is ERROR or Edwards25519.encode(point) != encoded:
        return ERROR
    if multiply(8, point) == Edwards25519.identity:
        return ERROR
    return point


function verifyExactBytes(ed25519PublicKey, protocolDefinedBytes, signature):
    if length(ed25519PublicKey) != 32 or length(signature) != 64:
        return false
    A = parseVerificationPublicKey(ed25519PublicKey)
    Rbytes = signature[0 .. 31]
    R = parseVerificationPublicKey(Rbytes)  // Same point rules for R.
    S = unsignedLittleEndianInteger(signature[32 .. 63])
    if A is ERROR or R is ERROR or S >= ED25519_GROUP_ORDER_L:
        return false
    k = unsignedLittleEndianInteger(
        SHA512(Rbytes || ed25519PublicKey || protocolDefinedBytes)
    ) mod ED25519_GROUP_ORDER_L
    return multiply(S, ED25519_BASE_POINT) == add(R, multiply(k, A))
```

The helpers express the [mandatory verification profile](../README.md#ed25519-verification-profile).
`ED25519_GROUP_ORDER_L` and `ED25519_BASE_POINT` have the values defined there.
Use an established cryptographic implementation with these exact checks; the
arithmetic above specifies acceptance, not a request to implement curve math.
Malformed inputs and decoder failures return false at the verification boundary.

The calling operation defines the exact bytes and domain separation. These
helpers do not invent a generic signing serialization or challenge protocol.
CR2SE connection identity proof uses the role-bound, fresh transcript in
[Transport specification](../../transport/pseudocode/README.md).

---

## 6. Safe X25519 key agreement

```text
function safeX25519(privateKeyHandleOrBytes, remotePublicKey):
    requireExactLength(
        remotePublicKey,
        X25519_PUBLIC_KEY_SIZE,
        "remote X25519 public key"
    )

    transferred = false
    try:
        shared = X25519(privateKeyHandleOrBytes, remotePublicKey)
        if shared is ERROR:
            raise InvalidKeyAgreement
        requireExactLength(shared, X25519_SHARED_SECRET_SIZE, "X25519 shared result")

        accumulator = 0
        for each byte in shared:
            accumulator = accumulator OR byte
        if accumulator == 0:
            raise InvalidKeyAgreement("all-zero X25519 shared result")

        transferred = true
        return shared  // Ownership moves to the caller's active cleanup guard.
    finally:
        if not transferred:
            securelyEraseTemporaryCopies(shared if assigned)
```

The all-zero test is mandatory even if the cryptographic library reports no
error. It should be implemented without data-dependent early exit.

---

## 7. Exact identity-encryption KDF

```text
function makeIdentityEncryptionKdfContext(
    recipientId,
    recipientPersistentPublicKey,
    ephemeralPublicKey,
    nonce
):
    requireExactLength(recipientId, CR2SE_ID_SIZE, "recipient CR2SE ID")
    requireExactLength(
        recipientPersistentPublicKey,
        X25519_PUBLIC_KEY_SIZE,
        "recipient X25519 public key")
    requireExactLength(
        ephemeralPublicKey,
        X25519_PUBLIC_KEY_SIZE,
        "ephemeral X25519 public key")
    requireExactLength(nonce, XCHACHA_NONCE_SIZE, "XChaCha20 nonce")

    context = byte(ENCRYPTION_FORMAT_VERSION)
        || byte(KEY_AGREEMENT_X25519)
        || byte(KEY_DERIVATION_HKDF_SHA256)
        || byte(AEAD_XCHACHA20_POLY1305)
        || recipientId
        || recipientPersistentPublicKey
        || ephemeralPublicKey
        || nonce

    assert length(context) == 124
    return context


function deriveIdentityEncryptionKey(sharedSecret, kdfContext):
    requireExactLength(
        sharedSecret,
        X25519_SHARED_SECRET_SIZE,
        "X25519 shared secret")
    requireExactLength(kdfContext, 124, "identity-encryption KDF context")

    salt = SHA256(KDF_SALT_PREFIX || byte(0x00) || kdfContext)
    transferred = false
    try:
        prk = HKDF_SHA256.extract(salt, sharedSecret)
        info = KDF_INFO_PREFIX || byte(0x00) || kdfContext
        key = HKDF_SHA256.expand(prk, info, ENCRYPTION_KEY_SIZE)
        requireExactLength(key, ENCRYPTION_KEY_SIZE, "derived key")
        transferred = true
        return key  // Ownership moves to the caller's active cleanup guard.
    finally:
        securelyEraseTemporaryCopies(prk if assigned)
        if not transferred:
            securelyEraseTemporaryCopies(key if assigned)
```

The suite bytes, recipient ID, recipient persistent public key, ephemeral
public key, and nonce must appear in that exact order. ASCII prefixes
contribute no terminating zero; the displayed `0x00` separator is explicit.

---

## 8. Header construction

```text
function encodeIdentityEncryptionHeader(
    recipientId,
    recipientPersistentPublicKey,
    ephemeralPublicKey,
    nonce,
    sealedLength
):
    requireExactLength(recipientId, CR2SE_ID_SIZE, "recipient CR2SE ID")
    requireExactLength(
        recipientPersistentPublicKey,
        X25519_PUBLIC_KEY_SIZE,
        "recipient X25519 public key")
    requireExactLength(
        ephemeralPublicKey,
        X25519_PUBLIC_KEY_SIZE,
        "ephemeral X25519 public key")
    requireExactLength(nonce, XCHACHA_NONCE_SIZE, "XChaCha20 nonce")

    if sealedLength < POLY1305_TAG_SIZE or sealedLength > UINT64_MAX:
        raise InvalidSealedLength

    header = byte(ENCRYPTION_FORMAT_VERSION)
        || byte(KEY_AGREEMENT_X25519)
        || byte(KEY_DERIVATION_HKDF_SHA256)
        || byte(AEAD_XCHACHA20_POLY1305)
        || recipientId
        || recipientPersistentPublicKey
        || ephemeralPublicKey
        || nonce
        || uint64_be(sealedLength)

    assert length(header) == ENCRYPTED_OBJECT_HEADER_SIZE
    return header
```

Offsets are fixed by concatenation: suite at 0, recipient ID at 4, recipient
key at 36, ephemeral key at 68, nonce at 100, and sealed length at 124.

---

## 9. Header and encrypted-object parsing

```text
function decodeIdentityEncryptionHeader(headerBytes):
    requireExactLength(
        headerBytes,
        ENCRYPTED_OBJECT_HEADER_SIZE,
        "identity-encryption header"
    )

    if headerBytes[0] != ENCRYPTION_FORMAT_VERSION:
        raise UnsupportedEncryptionFormat
    if headerBytes[1] != KEY_AGREEMENT_X25519:
        raise UnsupportedKeyAgreement
    if headerBytes[2] != KEY_DERIVATION_HKDF_SHA256:
        raise UnsupportedKeyDerivation
    if headerBytes[3] != AEAD_XCHACHA20_POLY1305:
        raise UnsupportedAuthenticatedEncryption

    header = EncryptionHeader {
        formatVersion: headerBytes[0],
        keyAgreementAlgorithm: headerBytes[1],
        keyDerivationAlgorithm: headerBytes[2],
        aeadAlgorithm: headerBytes[3],
        recipientId: headerBytes[4 .. 35],
        recipientX25519PublicKey: headerBytes[36 .. 67],
        ephemeralX25519PublicKey: headerBytes[68 .. 99],
        nonce: headerBytes[100 .. 123],
        sealedLength: uint64_be_at(headerBytes, 124),
        encoded: headerBytes
    }

    if header.sealedLength < POLY1305_TAG_SIZE:
        raise InvalidSealedLength

    return header


function parseEncryptedObject(encodedBytes, limits):
    if length(encodedBytes) < ENCRYPTED_OBJECT_HEADER_SIZE:
        raise TruncatedEncryptedObject

    if length(encodedBytes) > limits.maximumEncryptedObjectBytes:
        raise EncryptionResourceLimit

    headerBytes = encodedBytes[0 .. ENCRYPTED_OBJECT_HEADER_SIZE - 1]
    header = decodeIdentityEncryptionHeader(headerBytes)

    expectedTotal = checkedAdd(
        ENCRYPTED_OBJECT_HEADER_SIZE,
        header.sealedLength,
        limits.maximumEncryptedObjectBytes
    )

    if length(encodedBytes) != expectedTotal:
        raise InvalidEncryptedObjectLength

    plaintextLength = header.sealedLength - POLY1305_TAG_SIZE
    if plaintextLength > limits.maximumPlaintextBytes:
        raise EncryptionResourceLimit

    return ParsedEncryptedObject {
        header: header,
        sealed: encodedBytes[ENCRYPTED_OBJECT_HEADER_SIZE .. end]
    }
```

When reading a stream, read and validate the fixed header first, calculate the
checked total, enforce resource policy, and only then read exactly
`sealedLength` bytes. Do not search for a new boundary after malformed or
truncated cryptographic data.

---

## 10. Encrypt for one identity

```text
function encryptForIdentity(plaintext, recipientCredential, limits):
    recipient = validateEncryptionPublicCredential(recipientCredential)

    plaintextLength = length(plaintext)
    if plaintextLength > limits.maximumPlaintextBytes:
        raise EncryptionResourceLimit

    sealedLength = sealedLengthForPlaintext(plaintextLength)
    totalLength = checkedAdd(
        ENCRYPTED_OBJECT_HEADER_SIZE,
        sealedLength,
        limits.maximumEncryptedObjectBytes
    )

    try:
        ephemeralPair = X25519.generateKeyPair(CSPRNG)
        ephemeralPublic = X25519.canonicalPublicKeyBytes(ephemeralPair.publicKey)
        nonce = CSPRNG.randomBytes(XCHACHA_NONCE_SIZE)

        shared = safeX25519(
            ephemeralPair.privateKey,
            recipient.x25519PublicKey
        )
        context = makeIdentityEncryptionKdfContext(
            recipient.identityId,
            recipient.x25519PublicKey,
            ephemeralPublic,
            nonce
        )
        key = deriveIdentityEncryptionKey(shared, context)

        header = encodeIdentityEncryptionHeader(
            recipient.identityId,
            recipient.x25519PublicKey,
            ephemeralPublic,
            nonce,
            sealedLength
        )

        sealed = XChaCha20Poly1305.encrypt(
            key = key,
            nonce = nonce,
            plaintext = plaintext,
            authenticatedData = header
        )

        if length(sealed) != sealedLength:
            raise InternalCryptographicError

        encryptedObject = header || sealed
        assert length(encryptedObject) == totalLength
        return encryptedObject
    finally:
        securelyEraseTemporaryCopies(ephemeralPair.privateKey if assigned)
        securelyEraseTemporaryCopies(shared if assigned)
        securelyEraseTemporaryCopies(key if assigned)
```

The ephemeral pair and nonce are freshly generated for each object. The object
authenticates its full header but does not authenticate a sender.

---

## 11. Decrypt for the local identity

```text
function decryptForIdentity(encodedObject, localIdentity, limits):
    parsed = parseEncryptedObject(encodedObject, limits)
    header = parsed.header

    if header.recipientId != localIdentity.identityId:
        raise DecryptionFailure

    if header.recipientX25519PublicKey != localIdentity.encryptionPublicKey:
        raise DecryptionFailure

    try:
        shared = safeX25519(
            localIdentity.encryptionPrivateKeyHandle,
            header.ephemeralX25519PublicKey
        )
        context = makeIdentityEncryptionKdfContext(
            header.recipientId,
            header.recipientX25519PublicKey,
            header.ephemeralX25519PublicKey,
            header.nonce
        )
        key = deriveIdentityEncryptionKey(shared, context)

        // The cryptographic library writes into an internal staging buffer.
        // No plaintext is released to an application before tag verification.
        plaintext = XChaCha20Poly1305.decryptAndAuthenticate(
            key = key,
            nonce = header.nonce,
            sealed = parsed.sealed,
            authenticatedData = header.encoded
        )

        if plaintext is AUTHENTICATION_FAILURE:
            discardAnyStagedPlaintext()
            raise DecryptionFailure

        expectedPlaintextLength = header.sealedLength - POLY1305_TAG_SIZE
        if length(plaintext) != expectedPlaintextLength:
            discard plaintext
            raise DecryptionFailure

        return plaintext
    finally:
        securelyEraseTemporaryCopies(shared if assigned)
        securelyEraseTemporaryCopies(key if assigned)
```

Malformed structures, wrong recipients, invalid keys, all-zero shared results,
and failed tags all fail without accepted plaintext. An API may intentionally
map them to one externally visible decryption error to avoid exposing
unnecessary distinctions.

---

## 12. Self-encryption and intermediary transport

```text
function encryptForSelf(plaintext, localIdentityPublicCredential, limits):
    return encryptForIdentity(
        plaintext,
        localIdentityPublicCredential,
        limits
    )


function forwardEncryptedObject(encryptedObject, servicePolicy):
    // An intermediary treats it as opaque, untrusted bytes.
    enforce servicePolicy byte and storage limits
    store or transmit encryptedObject without requesting a private key
```

Self-encryption uses exactly the same envelope and algorithm. An intermediary
does not gain plaintext access and does not participate in the Alice-to-Bob
cryptographic relationship.

---

## 13. Large-data encryption units

The service or protocol that owns a large object defines chunk size, ordering,
complete-object representation, and incomplete-object behavior.

The simplest interoperable construction treats every chunk as an independent
version 1 identity-encryption object:

```text
function encryptIndependentChunks(plaintextStream, recipient, chunker, limits):
    for each bounded chunk from chunker.read(plaintextStream):
        encryptedUnit = encryptForIdentity(chunk, recipient, limits)
        yield encryptedUnit
```

Each call generates a new ephemeral pair, derived key, and nonce. The outer
service must still define boundaries and ordering; successful authentication
of individual chunks does not detect removal, duplication, or reordering by
itself.

A service may instead derive one key for several chunks, but it must define a
separate exact construction:

```text
require service specification defines:
    shared key-establishment envelope;
    exact per-chunk authenticated data;
    unique nonce assignment for every chunk under that key;
    chunk index and object identity binding;
    final length or end condition;
    duplicate, missing, reordered, and truncated chunk behavior;
    and when authenticated plaintext may be released.
```

The generic identity-encryption format must not be silently reinterpreted as
such a shared-key multi-chunk format.

---

## 14. General XChaCha20-Poly1305 use

```text
function encryptAuthenticatedUnit(key, plaintext, authenticatedData, allocator):
    requireExactLength(key, ENCRYPTION_KEY_SIZE, "XChaCha20 key")

    nonce = allocator.nextNonceForKey(key)
    requireExactLength(nonce, XCHACHA_NONCE_SIZE, "XChaCha20 nonce")
    require allocator guarantees nonce was never used with this key

    sealed = XChaCha20Poly1305.encrypt(
        key, nonce, plaintext, authenticatedData)
    return {nonce, sealed}


function decryptAuthenticatedUnit(key, nonce, sealed, authenticatedData):
    requireExactLength(key, ENCRYPTION_KEY_SIZE, "XChaCha20 key")
    requireExactLength(nonce, XCHACHA_NONCE_SIZE, "XChaCha20 nonce")

    plaintext = XChaCha20Poly1305.decryptAndAuthenticate(
        key, nonce, sealed, authenticatedData)
    if plaintext is AUTHENTICATION_FAILURE:
        discardAnyStagedPlaintext()
        raise DecryptionFailure
    return plaintext
```

Nonces are public, but reuse with one key is forbidden. A nonce sequence,
random generator, or other allocator is acceptable only when the operation
using it guarantees uniqueness for that key.

---

## 15. Confidential application communication

The Network handshake already authenticates identities, establishes separate
integrity keys, and leaves payloads visible. An application protocol that
needs confidentiality adds encryption above Network.

```text
function establishApplicationConfidentiality(connection, applicationProtocol):
    require connection has authenticated local and remote CR2SE IDs

    negotiation = applicationProtocol.performEncryptionNegotiation(connection)
    require negotiation exact messages and byte encoding are defined
    require negotiation is bound to:
        both authenticated identities
        this application operation or stream
        role and direction
        fresh key-establishment material

    keys = applicationProtocol.deriveDistinctDirectionalKeys(negotiation)
    require keys are distinct from:
        Network send and receive integrity keys
        Network nonce prefixes
        keys of unrelated application operations

    return applicationProtocol.newEncryptedChannel(keys)
```

Encryption version 1 does not define one universal application-negotiation
wire format. A protocol must not claim confidential communication until it has
defined key confirmation, authenticated context, nonce assignment, and any
required replay rejection.

The persistent identity-encryption envelope may be transported as ordinary
application bytes. It does not require changes to Network frame headers and
receives Network's mandatory connection tag like any other payload.

---

## 16. Key lifecycle and separation

```text
for one CR2SE identity:
    use one persistent Ed25519 pair for ID, signatures, and identity proof;
    use one persistent X25519 pair for identity encryption and key agreement;
    bind the X25519 public key with the exact Ed25519 binding signature;
    install the same two pairs on every node operating as that identity;
    keep both private keys secret;
    and do not infer a version 1 rotation protocol.

never:
    convert an Ed25519 key into the persistent X25519 key;
    use an Ed25519 key directly for X25519;
    use a raw X25519 shared result as an AEAD key;
    reuse Network integrity keys or nonces for content encryption;
    reuse an application-derived key for another purpose;
    transmit persistent or ephemeral private keys;
    log secret keys or unencrypted intermediate key material;
    or use deterministic test keys in production.
```

Loss of the Ed25519 private key loses identity control. Loss of only the
persistent X25519 private key loses decryption access for objects addressed to
that key. Compromise of only X25519 exposes confidentiality but does not by
itself produce Ed25519 identity signatures.

---

## 17. Failure and untrusted-data handling

```text
on invalid identity/public-key relation:
    reject credential

on invalid binding signature:
    reject encryption public key

on unsupported version or algorithm:
    reject without version 1 fallback

on malformed length or resource limit:
    reject before body allocation

on invalid X25519 operation or all-zero result:
    reject and erase intermediate secrets

on authenticated-decryption failure:
    release no plaintext

on a valid decryption:
    still treat plaintext as untrusted application input
```

A valid signature says only that the signing key authorized exact bytes. A
valid binding says only that the identity authorized an encryption key. Valid
ciphertext says only that its key holder can authenticate and decrypt it.
None of these facts grants trust, credit, permissions, or safety.

---

## 18. Minimum conformance cases

An implementation should test at least these cases against identity-key
binding, X25519 handling, HKDF derivation, envelope parsing, authenticated
encryption, key lifecycle, and layer separation:

```text
binding over exactly ASCII("CR2SE-ENCRYPTION-KEY-V1") || X25519 public key;
valid binding and changes to every binding message, key, ID, and signature byte;
claimed ID derived from the supplied Ed25519 public key and mismatch rejection;
32-byte Ed25519 and X25519 public keys and 64-byte Ed25519 signatures;
fresh persistent X25519 generation independent of Ed25519 generation;
same two persistent pairs installed on multiple nodes of one identity;
different signed X25519 keys for one ID detected as an unsupported conflict;
X25519 library errors and an explicitly all-zero 32-byte shared result;
KDF suite bytes in exact order and exact 124-byte context;
exact salt and HKDF info prefix bytes with explicit 0x00 separators;
recipient ID, recipient public key, and ephemeral public key mutations;
empty plaintext producing sealed length 16;
uint64 sealed-length boundaries and checked total-length overflow;
header truncation at every byte and sealed-body truncation;
unknown format, key-agreement, KDF, and AEAD identifiers;
recipient ID and persistent-key mismatch before decryption;
complete 132-byte header used as authenticated data;
modification of every header, ciphertext, and tag byte;
wrong private key, nonce, ephemeral key, and authenticated header;
no plaintext released before successful tag verification;
fresh ephemeral pair and 24-byte CSPRNG nonce for every encrypted object;
self-encryption using the ordinary recipient workflow;
opaque intermediary forwarding without private-key access;
large independent units using distinct keys and nonces;
shared-key chunk constructions rejected unless their service defines exact
    nonces, authenticated data, order, and completeness;
Network integrity keys and content-encryption keys never reused;
and private signing, persistent encryption, and ephemeral keys never transmitted.
```

---

## 19. Fixed identity-encryption test vector

These deterministic values test the exact version 1 binding, KDF, header, and
AEAD construction. Private values are test data only and must never be used by
a real identity or encrypted object.

```text
recipient Ed25519 seed:
202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f

recipient Ed25519 public key:
29acbae141bccaf0b22e1a94d34d0bc7361e526d0bfe12c89794bc9322966dd7

recipient CR2SE ID:
774504edf47bae28a4f1154bf4516ae74f090ee5b032464586d0c21d24d9b249

recipient raw persistent X25519 private-key input:
606162636465666768696a6b6c6d6e6f707172737475767778797a7b7c7d7e7f

recipient persistent X25519 public key:
675dd574ed7789310b3d2e7681f3790b466c773b1521fecf36577958371ea52f

encryption-key binding signature:
180898213e453d8621f8168c6fb893bfd6603c3d4e7ef980825f69c0646187ef
7855b8538f6314fbe6da2e7ba5bb90fd34dbf94793784ef9d434b2586a1f4a00

raw ephemeral X25519 private-key input:
404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f

ephemeral X25519 public key:
79a631eede1bf9c98f12032cdeadd0e7a079398fc786b88cc846ec89af85a51a

X25519 shared secret:
d6fb939511b2381bc8599b4b8edc5968829450dfd7a87aebe78a703cd04cd54e

KDF context:
01010101774504edf47bae28a4f1154bf4516ae74f090ee5b032464586d0c21d2
4d9b249675dd574ed7789310b3d2e7681f3790b466c773b1521fecf3657795837
1ea52f79a631eede1bf9c98f12032cdeadd0e7a079398fc786b88cc846ec89af
85a51a808182838485868788898a8b8c8d8e8f9091929394959697

HKDF salt:
0226a47d56431007781920ad9af236cb362e3b47863e49c71ddb97d679e437ec

HKDF extract PRK:
6efc6962876f9ae5ec0405a32814395f7a275c3cc8654ee71a9bea39815c5e5c

derived encryption key:
15fdb51cf1366090a57b5235d5af7b3fb5e7b844f4e38d8f39719fbdbe99d92e

nonce:
808182838485868788898a8b8c8d8e8f9091929394959697

plaintext ASCII("CR2SE identity encryption test"):
4352325345206964656e7469747920656e6372797074696f6e2074657374

132-byte header:
01010101774504edf47bae28a4f1154bf4516ae74f090ee5b032464586d0c21d2
4d9b249675dd574ed7789310b3d2e7681f3790b466c773b1521fecf3657795837
1ea52f79a631eede1bf9c98f12032cdeadd0e7a079398fc786b88cc846ec89af
85a51a808182838485868788898a8b8c8d8e8f9091929394959697000000000000
002e

sealed ciphertext plus tag (46 bytes):
e63419659e5234ee137b4d1f4efdb8db29a209310386ccfca7137f7c55cb440a
b2354765a9e06b466aef06931940
```

Whitespace and line breaks in these hexadecimal displays are not bytes. The
header's final eight bytes encode sealed length 46 as `uint64_be`. Decryption
must recover the exact 30-byte plaintext above.
