# 0003: Ed25519 weak-point validation is not specified

- Status: resolved
- Affected CR2SE modules: Cryptography, Identity and signature-consuming modules
- Upstream revision: [80f6795864ecf060e23482ee42909da4a649ecf0](https://github.com/beothorn/CR2SE/tree/80f6795864ecf060e23482ee42909da4a649ecf0)
- Specification: [algorithms and key separation](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/cryptography/README.md#algorithms-and-key-separation)
- Pseudocode: [general signing boundary](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/cryptography/pseudocode/README.md#5-general-signing-and-identity-proof-boundary)
- Upstream issue/fix: [CR2SE #6](https://github.com/beothorn/CR2SE/issues/6)

## Problem and evidence

CR2SE specifies ordinary Ed25519 and sizes, but does not specify rejection of
small-order public points or signature R points. The pseudocode delegates parsing
and verification to unspecified library operations. Ordinary and hardened
verifiers can therefore disagree on the same mathematically decodable key and
signature. Honest key generation does not validate attacker-supplied public keys.

## Minimal reproduction

Use public key A = `01` followed by 31 zero bytes (the Edwards identity point).
Use signature R = the same 32 bytes, followed by S = 32 zero bytes. Verify the
message ASCII("first"), then ASCII("different"), using the same signature.
For either message the uncofactored equation [S]B = R + [H(R,A,M)]A is identity
on both sides; a verifier with explicit small-order rejection rejects instead.
This ambiguity also applies when a transcript or binding is the signed message.

The executable regression
[weak_identity_point_signature_demonstrates_verification_ambiguity](../src/modules/cryptography/primitives/signature.rs)
confirms ordinary verification accepts both messages while strict verification
rejects. Run `cargo test --locked weak_identity_point_signature_demonstrates_verification_ambiguity`.
The fixture and group-equation argument are language-independent; the Rust test
is evidence, not the reason the protocol needs a rule.

## Expected behavior

Every CR2SE implementation should reach the same validation decision for received
public keys and signatures, including weak points. The specification must select
that decision explicitly rather than relying on backend defaults.

## Actual or ambiguous behavior

Accepting the verification equation and rejecting weak points give different
outcomes for identical inputs, while both can be selected under the stated
ordinary-Ed25519 guidance. The fixed valid vectors do not resolve this case.

## Impact

Implementations disagree on malformed/unacceptable credentials and identity
proofs. Permissive handling allows a degenerate claimed identity to pass proof
checks without demonstrating ownership of a normally generated private key.
This does not forge signatures for an existing honestly generated identity.

## Proposed protocol correction

Specify a verification profile including rejection of small-order public keys
and R points, scalar bounds, point decoding/canonicality, the verification equation
and consistent single/batch semantics. Add exact negative vectors, starting with
this fixture, to Cryptography and Transport acceptance cases. Clarify compatibility
for implementations that previously delegated to permissive library defaults.

## TalkSphere handling

TalkSphere uses ed25519-dalek's strict verification and rejects this fixture.
This is an explicit local rejection policy pending upstream clarification; it
changes acceptance of degenerate signatures, not honest generated signatures or
wire serialization. Passing valid vectors is not proof that the unresolved
edge-case validation profile is interoperable with all other implementations.

## Resolution

Resolution verified at upstream revision [d40b6278f24430e814d7fa82094f57261bfcac9f](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/cryptography/README.md#ed25519-verification-profile).
The specification now requires canonical decoding, small-order rejection, scalar bounds and the exact uncofactored equation; the acceptance suite supplies negative vectors and applies public-key validation before proof verification.

Verified by reviewing the specification, pseudocode and acceptance cases; this
is protocol-source verification, not execution of the new tests in TalkSphere.
The [Identity plan](../plans/3_Identity_module/PLAN.md) schedules the dependency
alignment and lifecycle evidence. Historical implementation evidence above
remains tied to its original baseline.
