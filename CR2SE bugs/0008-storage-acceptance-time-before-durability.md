# 0008: Storage timestamps precede durable acceptance

- Status: reported upstream
- Affected CR2SE module(s): Storage, Services composition/recovery
- Upstream revision: [d40b6278f24430e814d7fa82094f57261bfcac9f](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f)
- Specification/contract/pseudocode/vector sections: [Storage acceptance](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/README.md#acceptance), [providerStore timestamp and commit](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/pseudocode/README.md#L367-L391), [durable coordination](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/ModuleContracts.md#durable-effects-across-process-boundaries)
- Upstream issue/fix: https://github.com/beothorn/CR2SE/issues/11

## Problem and evidence

Storage section 5 assigns acceptance time when all five acceptance conditions,
including durable bytes and recorded lease metadata, have become true. Expiration
is acceptance time plus requested retention. The pseudocode instead samples the
clock, constructs expiration and generates an ID before commitReceivingAsActive.
A slow successful commit consumes paid retention before the lease is durably
accepted. No programming-language, ownership or filesystem-specific assumption
is involved.

The shared local coordination rules additionally stage complete results/effects
before preparation and decision delivery, while staged state is not an accepted
obligation. A frozen timestamp in a prepared result can also precede a delayed
commit decision. This composition case needs clarification alongside the direct
pseudocode correction.

## Minimal reproduction

Use a valid 3-byte object, retentionSeconds=10 and continuous normal UTC progress.
Inject the delay in the durable-store port; no real sleep or network failure is
necessary.

| Event | Provider UTC | Result |
|---|---:|---|
| Complete upload/hash validation | 1000 | Bytes remain provisional. |
| Pseudocode samples times | 1000 | acceptedAt=1000; expiresAt=1010. |
| Durable byte/metadata commit takes 20 seconds | 1000–1020 | Acceptance conditions are not all true yet. |
| Commit succeeds and output is returned | 1020 | The successful lease is already expired. |

Applying the normative post-durability rule gives acceptedAt=1020 and
expiresAt=1030. The two readings differ in the returned times and available
retention, even with honest peers and identical clocks.

## Expected behavior

The full accepted retention begins at the specified acceptance boundary, and
restart/recovery retains the same authoritative successful times and output.

## Actual or ambiguous behavior

The supplied direct pseudocode returns precommit timestamps. Shared preparation
also requires an explanation of how immutable staged output relates to a later
acceptance timestamp. Sampling after an earlier byte flush alone does not resolve
metadata commitment or delayed coordinator delivery.

## Impact

An implementation following the pseudocode can report and charge a successful
lease with shortened or zero remaining accepted retention. Different readings
produce different externally visible times and renewal/expiration behavior.

## Proposed protocol correction

Define the acceptance linearization point and a language-independent durable
storage/coordination contract assigning and retaining timestamps there. Reconcile
commit-time timestamp authorization/result finalization with exact prepared
effects and complete results, or consistently revise the acceptance definition.
Add slow-durability, delayed-decision and restart acceptance cases that verify
full retention and exact original result recovery.

## TalkSphere handling

Existing Storage work implements reusable validation, assessment, persistence
and participant primitives. Step 17's actual paid lease creation remains unchecked
pending clarification; no paid Storage provider is advertised. We have not
chosen a new timing rule or claimed that precommit timestamps satisfy the
post-durability requirement. The issue is distinct from existing paid-check
response-loss issue #10.

## Resolution

Unresolved as of 2026-11-17. Open and closed issues were searched for acceptedAt,
retention and durable acceptance before filing; no covering issue was found.
The GitHub create response confirmed issue #11 and its URL.
