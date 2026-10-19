# 0004: Secret cleanup guards start after fallible secret acquisition

- Status: open
- Affected CR2SE module: Cryptography pseudocode
- Upstream revision: [80f6795864ecf060e23482ee42909da4a649ecf0](https://github.com/beothorn/CR2SE/tree/80f6795864ecf060e23482ee42909da4a649ecf0)
- Specification: [decryption and validation](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/cryptography/README.md#decryption-and-validation)
- Pseudocode: [key generation](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/cryptography/pseudocode/README.md#3-identity-encryption-key-generation-and-binding), [KDF](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/cryptography/pseudocode/README.md#7-exact-identity-encryption-kdf), [encryption](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/cryptography/pseudocode/README.md#10-encrypt-for-one-identity)
- Upstream issue/fix: not filed upstream

## Problem and evidence

The normative contract requires temporary secrets to be erased when no longer
needed, including failure. encryptForIdentity generates the ephemeral pair and
then requests nonce randomness before entering the try/finally that erases the
pair. If randomness fails, that finally block is never entered. Similarly,
generateCompleteIdentityKeyMaterial generates both pairs and signs the binding
before its cleanup guard, and deriveIdentityEncryptionKey erases its PRK only
after HKDF expansion returns successfully.

## Minimal reproduction

1. Let ephemeral pair generation succeed.
2. Make the subsequent CSPRNG.randomBytes(24) operation fail.
3. Follow the displayed encryption control flow: execution exits before try,
   so securelyEraseTemporaryCopies(ephemeralPair.privateKey) is not executed.

This is a trace of published pseudocode, not an upstream executable run. The local
[encryption failure test](../src/modules/cryptography/envelope/encrypt.rs) exercises
this dependency failure; lifecycle tests additionally observe release. No particular
implementation language, exception mechanism or memory allocator is assumed.

## Expected behavior

Every acquired secret must be covered immediately by cleanup on all later exits,
including failures in subsequent entropy requests, derivation and binding signing.

## Actual or ambiguous behavior

The explicit cleanup statements do not cover those earlier exits. Some platforms
can supply automatic scoped cleanup, but the pseudocode does not require it and
its explicit guard placement suggests incomplete manual cleanup is sufficient.

## Impact

Implementations translating the control flow literally can retain ephemeral keys,
persistent-key staging or PRKs after dependency failure, contrary to the normative
lifecycle requirement. No wire incompatibility or remote key-recovery exploit is
claimed from this omission alone.

## Proposed protocol correction

Place the cleanup guard before the first secret-producing operation and erase each
assigned secret conditionally in finally, or explicitly require equivalent scoped
secret ownership. Guard PRK extraction/expansion too. Add fault cases after each
acquisition to CR-09, especially nonce failure after ephemeral generation.

## TalkSphere handling

Scoped zeroizing capabilities cover these exits from the moment of acquisition.
This follows the normative cleanup requirement and introduces no wire change.

## Resolution

Open against the pinned pseudocode; no upstream correction has been verified.
