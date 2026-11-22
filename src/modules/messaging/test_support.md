# Messaging test doubles

Test-only (`cfg(test)`), never part of the library. `FixedClock` shows, advances, sets and can be made to fail; `SequenceEntropy` yields a
deterministic, never repeating byte sequence and can be made to fail; `TestIdentity` has a real Ed25519 key derived from one seed byte, implements
the `Signer` port and can lose its signing ability. Different seeds give different identities. The in-memory placement store double is added
with the store port (step 12).
