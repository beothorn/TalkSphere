# 0002: Complete-handshake pseudocode derives keys before peer AUTH verification

- Status: open
- Affected CR2SE module(s): Transport
- Upstream revision: [8e848eb2e70ef7cd4490a753f9e647fdddcd99fd](https://github.com/beothorn/CR2SE/tree/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd)
- Specification: [section 5.3](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/README.md#53-session-key-derivation)
- Pseudocode: [section 7](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/pseudocode/README.md#7-complete-connection-handshake)
- Upstream issue/fix: not filed upstream

## Problem and evidence

The normative construction places X25519 shared-secret calculation after the
peer's AUTH verifies. performHandshake instead calls deriveSessionMaterial
immediately after HELLO, before signing/sending local AUTH and before receiving
or verifying the peer's AUTH. The two algorithms order these operations differently.

## Minimal reproduction

Follow performHandshake with a correctly shaped remote HELLO followed by an
invalid AUTH signature. The pseudocode calls X25519 and HKDF before discovering
the invalid signature. Following specification section 5.3, those calls never
occur for that peer. This is a trace of the published algorithm, not an upstream
executable run, and it is independent of implementation language.

## Expected behavior

Compute shared/session material only after successful remote AUTH verification,
as specified, while still sending local AUTH without waiting for the remote proof.

## Actual or ambiguous behavior

The pseudocode derives and retains session secrets for an as-yet-unverified
peer and assigns send material earlier than the normative sequence.

## Impact

Implementations disagree on primitive ordering, secret lifetime and which
failure is observed first when both signature and agreement are invalid. This
also complicates acceptance tests that assert no key derivation before proof
verification. No cryptographic exploit or wire-byte difference is asserted.

## Proposed protocol correction

Move derivation after successful peer signature verification and assign both
directional keys there. Track the outbound AUTH-to-tagged boundary separately;
send no application frames before verifying the peer. If an early error cannot
be safely authenticated after local AUTH, close without a response as section 12
already permits. Alternatively, explicitly amend the normative timing if early
derivation is intended and clarify its lifecycle requirements.

## TalkSphere handling

Handshake state follows the normative post-verification derivation order.
The strict failure script in [handshake tests](../src/modules/transport/handshake/state/tests.rs)
expects no agreement call after signature rejection. Run
`cargo test --locked expected_identity_signature_and_agreement_failures_never_emit_context`.
This follows the specification and is not a new protocol extension.

## Resolution

Open against the pinned revision; no upstream correction has been verified.
